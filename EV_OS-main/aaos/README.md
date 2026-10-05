# HELIOS AAOS APK bundle

User-provided APK artifacts copied into `aaos/apks/`. The filenames are preserved exactly.

- APK count: 26 (including media, navigation, EV charger locators, and system apps)
- Total size: approximately 2.53 GB
- SHA-256 identity and duplicate tracking documented in `apk_manifest.json` and `apk_manifest.csv`.
- **Soong Build Integration:** An [Android.bp](file:///Users/brendannewton/Desktop/Hypercar/EV_OS-main/aaos/apks/Android.bp) file has been added to `aaos/apks/` using `android_app_import` rules, enabling seamless inclusion into the AOSP / AAOS product image via `PRODUCT_PACKAGES`.

### Included Prebuilt Apps:
- Navigation & Charging: Google Maps, PlugShare AAOS, EV Charger Locator, MyRadar
- Media & Audio: Amazon Music for Automotive, YouTube Music, TuneIn Radio, OpenRadio, Anghami, Amazon Prime Video
- System & Integration: Google Play Services, Play Store, Android Auto Templates Host, Home Assistant, Chrome, Android WebView

Before production use, verify provenance/signatures, licensing, target AAOS release, privileged permissions, architecture, and compatibility with the selected Android target.
