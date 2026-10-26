import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('translations', ROOT / 'scripts/generate_translations.py')

class TranslationContractTests(unittest.TestCase):
    def test_missing_keys_and_damaged_placeholders_are_rejected(self):
        module = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(module)
        source = {'received': {'en': 'Downloaded {count}', 'ru': 'Скачано {count}'}}
        with self.assertRaises(ValueError):
            module.validate_catalog(source, {}, 'es')
        with self.assertRaises(ValueError):
            module.validate_catalog(source, {'received': 'Descargado {wrong}'}, 'es')
        module.validate_catalog(source, {'received': 'Descargado {count}'}, 'es')
        limit = {'limit': {'en': 'At least 0.5 Mbit/s', 'ru': 'От 0,5 Мбит/с'}}
        module.validate_catalog(limit, {'limit': 'Au moins 0,5 Mbit/s'}, 'fr')
        with self.assertRaises(ValueError):
            module.validate_catalog(limit, {'limit': 'Au moins 0,1 Mbit/s'}, 'fr')

    def test_all_catalogs_and_embedded_output_are_current(self):
        module = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(module)
        source = json.loads((ROOT/'assets/locales/messages.json').read_text(encoding="utf-8"))
        self.assertEqual(len({(v['en'], v['ru']) for v in source.values()}), len(source))
        for tag in module.LANGUAGES:
            module.validate_catalog(source, json.loads((ROOT/'assets/locales'/f'{tag}.json').read_text(encoding="utf-8")), tag)
        self.assertEqual(module.generate(ROOT), (ROOT/'src/ui/translations_generated.rs').read_text(encoding="utf-8"))
        self.assertFalse(module.missing_source_pairs(ROOT, source))
