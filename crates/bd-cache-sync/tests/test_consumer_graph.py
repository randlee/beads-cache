"""Planted provenance/server leaks and real owned-subprocess timeout proof."""
import importlib.util
from pathlib import Path
import sys
import unittest

spec = importlib.util.spec_from_file_location('consumer_graph', Path(__file__).with_name('check_consumer_graph.py'))
graph = importlib.util.module_from_spec(spec)
spec.loader.exec_module(graph)


def fixture():
    packages = [{'id': n, 'name': n, 'version': '0.2.0',
                 'source': 'registry+https://github.com/rust-lang/crates.io-index'}
                for n in ['consumer', 'bd-cache-sync', 'sc-cron', 'sc-command']]
    nodes = [{'id': p['id'], 'features': [], 'deps': []} for p in packages]
    for source, target in [(0, 1), (1, 2), (2, 3)]:
        nodes[source]['deps'].append({'pkg': nodes[target]['id'], 'dep_kinds': [{'kind': None}]})
    return {'packages': packages, 'resolve': {'root': 'consumer', 'nodes': nodes}}


class GraphTest(unittest.TestCase):
    def test_exact_published_graph_passes(self):
        self.assertEqual(graph.violations(fixture()), [])

    def test_only_the_approved_scheduler_may_introduce_contract_logging(self):
        for source in (0, 1):
            for kind in (None, 'build'):
                with self.subTest(source=source, kind=kind):
                    bad = fixture()
                    bad['resolve']['nodes'][source]['deps'].append(
                        {'pkg': 'sc-command', 'dep_kinds': [{'kind': kind}]})
                    self.assertTrue(any('-> sc-command' in e for e in graph.violations(bad)))

    def test_unapproved_revision_source_path_or_version_fails_closed(self):
        revision = 'c3ce3d1b271b54fbb47efaef57ee1563a390cd9e'
        source = f'git+https://github.com/randlee/sc-runtime?rev={revision}#{revision}'
        wrong = '0' * 40
        rejected = [
            None,
            source,
            'path+file:///checkout',
            'registry+https://invalid.example/index',
            source.replace(revision, wrong),
            source.replace('?rev=' + revision, '?rev=' + wrong),
            source.replace('#' + revision, '#' + wrong),
            source.replace('github.com/randlee/', 'github.com/other/'),
            source.replace('?rev=' + revision, '?branch=integrate/phase-c'),
            source + 'extra',
        ]
        for package_index in (2, 3):
            for unapproved in rejected:
                with self.subTest(package=package_index, source=unapproved):
                    bad = fixture()
                    bad['packages'][package_index]['source'] = unapproved
                    self.assertTrue(any('must use published' in e for e in graph.violations(bad)))
            for version in ('0.1.1', '0.2.1'):
                bad = fixture()
                bad['packages'][package_index]['version'] = version
                self.assertTrue(any('must use published' in e for e in graph.violations(bad)))

    def test_mixed_sources_do_not_split_nominal_contracts(self):
        for index in (2, 3):
            bad = fixture()
            bad['packages'][index].update(version='0.2.0', source='git+https://github.com/randlee/sc-runtime#old')
            self.assertTrue(any('same-source' in e for e in graph.violations(bad)))
        duplicate = fixture()
        extra = dict(duplicate['packages'][3], id='another-command')
        duplicate['packages'].append(extra)
        duplicate['resolve']['nodes'].append({'id': extra['id'], 'features': [], 'deps': []})
        duplicate['resolve']['nodes'][2]['deps'].append(
            {'pkg': extra['id'], 'dep_kinds': [{'kind': None}]})
        self.assertTrue(any('multiple sc-command package identities' in e for e in graph.violations(duplicate)))

    def test_forbidden_assembly_and_feature_leaks_fail(self):
        for name in ('axum', 'rmcp', 'sc-runtime', 'sc-transport', 'clap',
                     'clap_builder', 'sc-command-macros', 'sc-task'):
            bad = fixture()
            bad['packages'][2]['name'] = name
            self.assertTrue(any(f'contains {name}' in e for e in graph.violations(bad)))
        for index, features in ((2, ('monty', 'test-utils', 'default')),
                                (3, ('server', 'client', 'macros', 'task-contract', 'default'))):
            for feature in features:
                bad = fixture()
                bad['resolve']['nodes'][index]['features'].append(feature)
                self.assertTrue(any('isolated consumer enables' in e for e in graph.violations(bad)))

    def test_multiple_sqlite_providers_still_fail(self):
        bad = fixture()
        for package in bad['packages'][2:]:
            package['links'] = 'sqlite3'
        self.assertTrue(any('multiple SQLite link providers' in e for e in graph.violations(bad)))

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
