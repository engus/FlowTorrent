"""A dependency waiver must stop applying when its reviewed graph changes."""
import copy
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("ringbuf_policy", Path(__file__).resolve().parents[1] / "check_ringbuf_exception.py")
policy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(policy)
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


class DependencyPolicyTests(unittest.TestCase):
    def graph(self):
        return {
            "packages": [
                {"id": "buffer", "name": "ringbuf", "version": "0.4.8", "source": REGISTRY},
                {"id": "transport", "name": "librqbit-utp", "version": "0.7.0", "source": REGISTRY},
                {"id": "app", "name": "flowtorrent", "version": "0.1.0", "source": None},
            ],
            "resolve": {"nodes": [
                {"id": "buffer", "dependencies": []},
                {"id": "transport", "dependencies": ["buffer"]},
                {"id": "app", "dependencies": ["transport"]},
            ]},
        }

    def test_audited_consumer_is_accepted(self):
        policy.check_scope(self.graph())

    def test_a_new_direct_consumer_requires_review(self):
        graph = self.graph()
        graph["resolve"]["nodes"][2]["dependencies"].append("buffer")
        with self.assertRaises(ValueError):
            policy.check_scope(graph)

    def test_version_or_source_changes_require_review(self):
        for index, key, value in [(0, "version", "0.5.2"), (1, "version", "0.8.0"), (1, "source", None)]:
            graph = copy.deepcopy(self.graph())
            graph["packages"][index][key] = value
            with self.assertRaises(ValueError):
                policy.check_scope(graph)
