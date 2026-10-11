# HELIOS AAOS APK bundle

User-provided APK artifacts copied into `aaos/apks/`. The filenames are preserved exactly.

- APK count: 26 (including media, navigation, EV charger locators, and system apps)
- Total size: approximately 2.53 GB
- SHA-256 identity and duplicate tracking documented in `apk_manifest.json` and `apk_manifest.csv`.
- **Soong Build Integration:** [`Android.bp`](./apks/Android.bp) defines one `android_app_import` module per unique APK. Add the desired module names to the product's `PRODUCT_PACKAGES`; the import file alone does not install them into an image.

### Included Prebuilt Apps:
- Navigation & Charging: Google Maps, PlugShare AAOS, EV Charger Locator, MyRadar
- Media & Audio: Amazon Music for Automotive, YouTube Music, TuneIn Radio, OpenRadio, Anghami, Amazon Prime Video
- System & Integration: Google Play Services, Play Store, Android Auto Templates Host, Home Assistant, Chrome, Android WebView

Duplicate downloads are recorded in the manifests but are not imported as separate modules. The APK binaries are local-only and excluded from Git; copy authorized APKs into `aaos/apks/` before building. Before production use, verify provenance/signatures, licensing, target AAOS release, privileged permissions, architecture, and compatibility with the selected Android target.
