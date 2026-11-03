"""Compare the experimental compact profile with unchanged release on one runner."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

SCENARIOS = ['empty', 'active5', 'paused20', 'hidden5', 'cycles5']
METRICS = ('resident_bytes', 'private_bytes', 'physical_footprint_bytes')


def analyze_samples(path, scenario, expected_samples):
    with Path(path).open(newline='') as source:
        rows = list(csv.DictReader(source))
    if len(rows) != expected_samples or len(rows) < 2:
        raise ValueError(f'{scenario}: missing samples')
    elapsed = [float(r['elapsed_seconds']) for r in rows]
    cpu = [int(r['cpu_time_ns']) for r in rows]
    checked = [int(r['checked_bytes']) for r in rows]
    if any(b <= a for a, b in zip(elapsed, elapsed[1:])):
        raise ValueError('Elapsed time must increase')
    for values in (cpu, checked):
        if any(v < 0 for v in values) or any(b < a for a, b in zip(values, values[1:])):
            raise ValueError('Counters must not move backwards')
    visible = {r['visible'] for r in rows}
    expected = {'false'} if scenario == 'hidden5' else {'true', 'false'} if scenario == 'cycles5' else {'true'}
    if visible != expected:
        raise ValueError(f'{scenario}: unexpected visibility {visible}')
    progress = checked[-1] - checked[0]
    if scenario.endswith('5') and progress <= 0:
        raise ValueError(f'{scenario}: no positive transfer control')
    duration = elapsed[-1] - elapsed[0]
    result = {'scenario': scenario, 'samples': len(rows), 'progress_delta_bytes': progress,
              'transfer_bytes_per_second': progress / duration,
              'cpu_percent_one_core': 100 * (cpu[-1] - cpu[0]) / 1e9 / duration}
    for metric in METRICS:
        values = [int(r[metric]) for r in rows if r[metric]]
        if values and len(values) != len(rows):
            raise ValueError(f'{metric}: partially missing metric')
        result[metric] = {'median': statistics.median(values), 'peak': max(values)} if values else None
    return result


def comparison_order():
    return ['release', 'compact', 'compact', 'release']


def spread(values):
    return {'median': statistics.median(values), 'min': min(values), 'max': max(values)}


def compare_observations(observations):
    comparisons = []
    for scenario in dict.fromkeys(o['scenario'] for o in observations):
        item = {'scenario': scenario}
        for profile in ('release', 'compact'):
            selected = [o for o in observations if o['scenario'] == scenario and o['profile'] == profile]
            if len(selected) != 2:
                raise ValueError('Comparison requires two observations per profile and scenario')
            data = {'cpu_percent_one_core': spread([o['cpu_percent_one_core'] for o in selected])}
            for metric in METRICS:
                values = [o[metric] for o in selected]
                if all(v is None for v in values):
                    data[metric] = None
                elif any(v is None for v in values):
                    raise ValueError('Metric availability changed between observations')
                else:
                    data[metric] = dict(spread([v['median'] for v in values]), peak=max(v['peak'] for v in values))
            item[profile] = data
        baseline = item['release']['cpu_percent_one_core']['median']
        item['cpu_ratio'] = item['compact']['cpu_percent_one_core']['median'] / baseline if baseline else None
        comparisons.append(item)
    return comparisons


def logged(command, path):
    print('Running: ' + ' '.join(map(str, command)), flush=True)
    started = time.monotonic()
    with path.open('w', encoding='utf-8') as output:
        result = subprocess.run(command, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode:
        print('\n'.join(path.read_text(encoding='utf-8', errors='replace').splitlines()[-80:]), flush=True)
        result.check_returncode()
    return round(time.monotonic() - started, 3)


def cargo_command(action, profile, target, *arguments):
    command = ['cargo', action, '--locked', '--profile', profile, '-j', '2']
    if target != 'native':
        command.extend(['--target', target])
    command.extend(arguments)
    if sys.platform == 'darwin':
        command.extend(['--config', f'profile.{profile}.build-override.strip="none"'])
    return command


def observe(binary, folder, scenario, seconds, profile, index):
    start = time.monotonic()
    subprocess.run([str(binary), str(folder), scenario, str(seconds)], check=True,
                   timeout=max(360, seconds + 240))
    result = analyze_samples(folder/'samples.csv', scenario, seconds)
    environment_path = folder/'environment.json'
    environment = json.loads(environment_path.read_text())
    environment.update(profile=profile, binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                       comparison_observation=index)
    environment_path.write_text(json.dumps(environment, indent=2) + '\n')
    result.update(profile=profile, observation=index, probe_wall_seconds=time.monotonic()-start,
                  binary_sha256=environment['binary_sha256'], environment=str(environment_path))
    print(json.dumps(result), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', default='native')
    parser.add_argument('--seconds', type=int, default=15)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--scenarios', nargs='+', default=SCENARIOS, choices=SCENARIOS)
    parser.add_argument('--compare-profile', choices=('compact', 'none'), default='compact',
                        help='This experimental branch defaults to paired measurements; none retains the original release-only probe')
    args = parser.parse_args()
    if not 10 <= args.seconds <= 86400:
        parser.error('seconds must be 10..86400')
    root = (args.output or Path(os.environ['RUNNER_TEMP'])/'flowtorrent-resources').resolve()
    root.mkdir(exist_ok=False)
    suffix = '.exe' if sys.platform == 'win32' else ''
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--no-deps', '--format-version', '1'], text=True))
    target_dir = Path(metadata['target_directory'])
    if args.target != 'native':
        target_dir = target_dir/args.target
    binaries = {p: (target_dir/p/'examples'/('measure_resources'+suffix)).resolve() for p in ('release','compact')}
    if args.compare_profile == 'none':
        summary = []
        for scenario in args.scenarios:
            summary.append(observe(binaries['release'], root/scenario, scenario, args.seconds, 'release', 0))
            (root/'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        return
    report = {'schema':1, 'order':comparison_order(), 'builds':{}, 'observations':[],
              'comparisons':[], 'notes':[
                  'Same-runner controlled local swarm; 20-second warmup; two observations per profile.',
                  '655360 B/s shared download cap: CPU and memory at matched work, not maximum throughput.',
                  'Instrumented client omits IPC and notifications; Windows probe uses dynamic CRT, installed package static CRT.',
                  'No physical acceptance, overnight result or statistical-significance claim.'
              ]}
    (root/'summary.json').write_text(json.dumps(report, indent=2)+'\n')
    for profile in ('release','compact'):
        report['builds'][profile] = {'seconds':logged(cargo_command('build',profile,args.target,'--bin','flowtorrent','--example','measure_resources'),root/f'{profile}-build.log'),
                                    'binary_bytes':binaries[profile].stat().st_size,
                                    'binary_sha256':hashlib.sha256(binaries[profile].read_bytes()).hexdigest()}
    if args.scenarios == SCENARIOS:
        logged(cargo_command('test','compact',args.target,'--all-targets'),root/'compact-tests.log')
        if sys.platform == 'darwin':
            logged(cargo_command('run','compact',args.target,'--example','macos_lifecycle_probe'),root/'compact-lifecycle.log')
    for scenario in args.scenarios:
        for index, profile in enumerate(comparison_order()):
            folder = root/f'{scenario}-{index}-{profile}'
            report['observations'].append(observe(binaries[profile],folder,scenario,args.seconds,profile,index))
            (root/'summary.json').write_text(json.dumps(report,indent=2)+'\n')
    report['comparisons'] = compare_observations(report['observations'])
    (root/'summary.json').write_text(json.dumps(report,indent=2)+'\n')


if __name__ == '__main__':
    main()
