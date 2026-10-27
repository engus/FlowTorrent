import importlib.util
import json
from pathlib import Path
import unittest
import tempfile

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

    def test_desktop_workspace_sources_are_scanned_for_missing_messages(self):
        module = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root/'apps/desktop/src/ui/example.rs'
            source.parent.mkdir(parents=True)
            source.write_text('locale.text("test-ru", "test-en")', encoding="utf-8")
            self.assertEqual(module.missing_source_pairs(root, {}), [("test-ru", "test-en")])

    def test_slint_conditional_translations_are_scanned_after_move(self):
        module = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root/'apps/desktop/ui/example.slint'
            source.parent.mkdir(parents=True)
            source.write_text('ru ? "conditional-ru" : "conditional-en"', encoding="utf-8")
            self.assertEqual(module.missing_source_pairs(root, {}), [("conditional-ru", "conditional-en")])

    def test_all_catalogs_and_embedded_output_are_current(self):
        module = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(module)
        source = json.loads((ROOT/'assets/locales/messages.json').read_text(encoding="utf-8"))
        self.assertEqual(len({(v['en'], v['ru']) for v in source.values()}), len(source))
        for tag in module.LANGUAGES:
            module.validate_catalog(source, json.loads((ROOT/'assets/locales'/f'{tag}.json').read_text(encoding="utf-8")), tag)
        self.assertEqual(module.generate(ROOT), (ROOT/'apps/desktop/src/ui/translations_generated.rs').read_text(encoding="utf-8"))
        self.assertFalse(module.missing_source_pairs(ROOT, source))
