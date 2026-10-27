[Русский](localization-ru.md) · [Documentation](../README.md)

# Localization

The approved desktop-oriented language set contains 14 languages. It was chosen for this project; it is not an objective ranking of worldwide language use. English and Russian remain the contextual source pair; 12 additional catalogs supply translations.

| Tag | Settings autonym | Language |
| --- | --- | --- |
| en | English | English |
| ru | Русский | Russian |
| es | Español | Spanish |
| pt | Português | Portuguese |
| fr | Français | French |
| de | Deutsch | German |
| it | Italiano | Italian |
| pl | Polski | Polish |
| tr | Türkçe | Turkish |
| ja | 日本語 | Japanese |
| ko | 한국어 | Korean |
| zh-CN | 简体中文 | Simplified Chinese |
| kk | Қазақша | Kazakh |
| uk | Українська | Ukrainian |

## Selection and fallback

Settings offers **System** and the 14 autonyms. A saved explicit language overrides system detection. System tags normalize underscores and case, then resolve by language. Unsupported locales fall back to English. Simplified Chinese is supported; Traditional Chinese tags containing `Hant`, `TW`, `HK` or `MO` fall back to English rather than being presented as a Traditional Chinese translation. Other unsupported language variants do not create new catalogs.

Paths, torrent/file names, magnet links, author identity and product names are not translated. Locale-aware formatting uses the application's existing unit and decimal conventions. Native OS dialogs, OS-owned menu items and upstream component credits can follow their own localization.

## Catalog workflow

- [messages.json](../../assets/locales/messages.json) maps stable message IDs to contextual `ru` / `en` pairs. The complete pair is the lookup context: equal English words may need different translations in different contexts.
- The 12 sibling language JSON files contain the same IDs with non-empty translated values.
- [generate_translations.py](../../scripts/generate_translations.py) validates matching keys, placeholders, numeric values and source coverage, then emits [static Rust tables](../../apps/desktop/src/ui/translations_generated.rs).
- Rust keeps the `Locale::text(ru, en)` call shape. The pure [Slint Translations global](../../apps/desktop/ui/translations.slint) delegates string lookup to the selected locale. Language changes apply after the settings transaction succeeds.

From the repository root:

```sh
python3 scripts/generate_translations.py
python3 scripts/generate_translations.py --check
python3 -m unittest discover -s scripts/tests -p 'test_translations.py' -v
```

For a new message, add its exact contextual pair to `messages.json`, translate the ID in every added-language catalog, generate the table and run the check. For a wording change, update the source pair and all affected translations together. Do not edit the generated Rust table directly. Catalogs are development inputs: the shipped app does not fetch translations, access the internet for localization or parse JSON catalogs at runtime. Missing runtime lookups fall back to English; the generator check is intended to catch missing catalog coverage before shipping.

## Acceptance limits

Catalog coverage and automated checks establish structural consistency. They do not establish linguistic accuracy or prove layout on every native platform. The new translations still need native-speaker acceptance, and a complete formal 14-language native UI matrix has not been completed. Review long labels, wrapping and elision, CJK/Kazakh/Ukrainian glyphs, both themes, resize behavior, keyboard/focus, errors, confirmation consequences and number/unit meaning. Record actual platform, build revision and result in validation evidence; never infer these checks from catalog generation alone.
