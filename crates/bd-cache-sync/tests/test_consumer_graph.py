"""Planted provenance/server leaks and real owned-subprocess timeout proof."""
import copy
import importlib.util
from pathlib import Path
import sys
import unittest

spec = importlib.util.spec_from_file_location('consumer_graph', Path(__file__).with_name('check_consumer_graph.py'))
graph = importlib.util.module_from_spec(spec)
spec.loader.exec_module(graph)


def fixture():
    packages = [{'id': n, 'name': n, 'version': '0.1.1',
                 'source': 'registry+https://github.com/rust-lang/crates.io-index'}
                for n in ['consumer', 'bd-cache-sync', 'sc-cron', 'sc-command']]
    nodes = [{'id': p['id'], 'features': [], 'deps': []} for p in packages]
    for source, target in [(0, 1), (1, 2), (2, 3)]:
        nodes[source]['deps'].append({'pkg': nodes[target]['id'], 'dep_kinds': [{'kind': None}]})
    return {'packages': packages, 'resolve': {'root': 'consumer', 'nodes': nodes}}


class GraphTest(unittest.TestCase):
    def test_only_the_published_scheduler_may_introduce_contract_logging(self):
        good = fixture()
        self.assertEqual(graph.violations(good), [])
        bad = copy.deepcopy(good)
        bad['resolve']['nodes'][1]['deps'].append({'pkg': 'sc-command', 'dep_kinds': [{'kind': 'build'}]})
        self.assertTrue(any('bd-cache-sync -> sc-command' in e for e in graph.violations(bad)))
        local = copy.deepcopy(good)
        local['packages'][2]['source'] = None
        self.assertTrue(any('must be published' in e for e in graph.violations(local)))

    def test_forbidden_standalone_http_mcp_and_server_features_fail(self):
        for name in ('axum', 'rmcp', 'sc-runtime', 'sc-transport', 'clap'):
            bad = fixture()
            bad['packages'][2]['name'] = name
            self.assertTrue(any(f'contains {name}' in e for e in graph.violations(bad)))
        server = fixture()
        server['resolve']['nodes'][3]['features'].append('server')
        self.assertTrue(any('sc-command/server' in e for e in graph.violations(server)))

    def test_dev_only_packages_are_not_consumer_normal_build_edges(self):
        meta = fixture()
        meta['packages'][2]['name'] = 'clap'
        meta['resolve']['nodes'][1]['deps'][0]['dep_kinds'][0]['kind'] = 'dev'
        self.assertEqual(graph.violations(meta), [])

    def test_owned_metadata_child_is_killed_and_reaped_at_deadline(self):
        with self.assertRaisesRegex(RuntimeError, 'owned child killed and reaped'):
            graph.metadata([sys.executable, '-I', '-c', 'import time; time.sleep(30)'], timeout=0.05)


if __name__ == '__main__':
    unittest.main()
