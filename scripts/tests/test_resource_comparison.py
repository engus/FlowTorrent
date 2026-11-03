"""Measurement validity using real CSV observations, including counterexamples."""
import csv
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import validate_resources as resources


class SampleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def samples(self, *, visible=('false','false','false'), progress=(0,100,200), cpu=(0,10_000_000,20_000_000)):
        path=self.root/'samples.csv'
        with path.open('w', newline='') as f:
            writer=csv.writer(f)
            writer.writerow(['elapsed_seconds','resident_bytes','private_bytes','physical_footprint_bytes','cpu_time_ns','checked_bytes','visible'])
            for t in range(3):
                writer.writerow([t,100+t,'','',cpu[t],progress[t],visible[t]])
        return path

    def test_real_samples_keep_missing_metrics_distinct_from_zero(self):
        result=resources.analyze_samples(self.samples(), 'hidden5', 3)
        self.assertEqual(result['progress_delta_bytes'],200)
        self.assertAlmostEqual(result['cpu_percent_one_core'],1.0)
        self.assertEqual(result['resident_bytes'],{'median':101,'peak':102})
        self.assertIsNone(result['private_bytes'])
        self.assertIsNone(result['physical_footprint_bytes'])

    def test_missing_samples_and_backward_counters_are_rejected(self):
        for changes, count in [({},4), ({'progress':(100,50,200)},3), ({'cpu':(100,50,200)},3)]:
            with self.subTest(changes=changes,count=count), self.assertRaises(ValueError):
                resources.analyze_samples(self.samples(**changes),'hidden5',count)

    def test_visibility_and_positive_transfer_are_controls(self):
        cases=[({'visible':('true','false','false')},'hidden5'),
               ({},'cycles5'), ({'progress':(0,0,0)},'hidden5')]
        for changes,scenario in cases:
            with self.subTest(changes=changes,scenario=scenario), self.assertRaises(ValueError):
                resources.analyze_samples(self.samples(**changes),scenario,3)
        resources.analyze_samples(self.samples(visible=('true','false','true')),'cycles5',3)

    def test_comparison_reports_ranges_and_does_not_divide_by_zero_cpu(self):
        observations=[]
        for profile,value in [('release',0),('compact',2),('compact',4),('release',0)]:
            observations.append({'profile':profile,'scenario':'empty','cpu_percent_one_core':value,
                                 'progress_delta_bytes':0, 'transfer_bytes_per_second':0, 'resident_bytes':{'median':100+value,'peak':120+value},
                                 'private_bytes':None,'physical_footprint_bytes':None})
        data=resources.compare_observations(observations)
        self.assertIsNone(data[0]['cpu_ratio'])
        self.assertEqual(data[0]['compact']['cpu_percent_one_core'],{'median':3,'min':2,'max':4})
        self.assertIsNone(data[0]['compact']['private_bytes'])
        self.assertEqual(resources.comparison_order(),['release','compact','compact','release'])

    def test_positive_but_unequal_transfer_cannot_look_like_a_cpu_speedup(self):
        observations=[]
        for profile,progress,cpu in [('release',(0,100,200),(0,100,200)),
                                     ('compact',(0,1,2),(0,1,2)),
                                     ('compact',(0,1,2),(0,1,2)),
                                     ('release',(0,100,200),(0,100,200))]:
            sample=resources.analyze_samples(self.samples(progress=progress,cpu=cpu),'hidden5',3)
            sample['profile']=profile
            observations.append(sample)
        result=resources.compare_observations(observations)[0]
        self.assertFalse(result['matched_work'])
        self.assertIsNone(result['cpu_ratio'])
        self.assertEqual(result['release']['transfer_bytes_per_second']['median'],100)
        self.assertEqual(result['compact']['transfer_bytes_per_second']['median'],1)
