# Changelog

## [0.2.1](https://github.com/TheChasman/CSpy/compare/v0.2.0...v0.2.1) (2026-04-03)


### Bug Fixes

* trigger build on release event, not push tag ([af7d7d8](https://github.com/TheChasman/CSpy/commit/af7d7d861b8c0848d6228304cf932c52deaf9921))

## [0.2.0](https://github.com/TheChasman/CSpy/compare/v0.1.0...v0.2.0) (2026-04-03)


### Features

* add auto-updater with quiet-hours restart ([fba92d2](https://github.com/TheChasman/CSpy/commit/fba92d29882203fbfbc797cbb617f8301f90505c))
* add burn rate tier and calculation functions ([998614d](https://github.com/TheChasman/CSpy/commit/998614d04562757f113a2b78443d77911f290b11))
* add DEV/PREV env badge to popover footer ([93ee40a](https://github.com/TheChasman/CSpy/commit/93ee40aec0250cba74e3b0a1ad3accbba528584b))
* add dev/prev/prod switch scripts and preview CI build ([cce9a6b](https://github.com/TheChasman/CSpy/commit/cce9a6b42ff85ae321c96ade1fb441b52e93962a))
* add version display bottom right of popover ([f590def](https://github.com/TheChasman/CSpy/commit/f590def2159ec2a7c22960cc1193b6de4d3d4ea0))
* regenerate tray icon on each poll with dynamic fill ([a194bcd](https://github.com/TheChasman/CSpy/commit/a194bcdafd1e8d44b8e6190f885cb6ed37a64df9))
* simplify popover to 5-hour only, add burn rate display ([fb8bc01](https://github.com/TheChasman/CSpy/commit/fb8bc0140eaf22da6f8f4ae9c140acfbd37252ac))


### Bug Fixes

* add exact-name pkill to catch open-launched app instances ([b4ae0c7](https://github.com/TheChasman/CSpy/commit/b4ae0c7f7e3bc86a43bde8c6feefc70ea02e3512))
* background cargo tauri dev so switch_dev returns the terminal ([630ac17](https://github.com/TheChasman/CSpy/commit/630ac17c2ffbb2e032a72b6362ed065d359d05e0))
* bump all fonts +2pt, handle 429 gracefully on manual refresh ([0700169](https://github.com/TheChasman/CSpy/commit/07001692f5e9518c795b4a77ea3412bf1fe0faf0))
* correct beforeDevCommand path — remove erroneous ../ prefix ([e8be5f3](https://github.com/TheChasman/CSpy/commit/e8be5f31fbe2c0c0a1aa3803f966576e94e3bb25))
* correct border bounds in icon renderer to prevent subtract overflow ([1582a6b](https://github.com/TheChasman/CSpy/commit/1582a6bfed5f08a67cad46de75e8c45d8ac56788))
* disable bump-patch-for-minor-pre-major so features bump minor version ([5635e8f](https://github.com/TheChasman/CSpy/commit/5635e8ff181685914cddfb8659c0b887abebdeb8))
* eliminate startup request clustering and handle 429 gracefully ([c6533f0](https://github.com/TheChasman/CSpy/commit/c6533f03379cec4a3b41555958536f600f7e9c91))
* kill debug binary (target/debug/cspy) to prevent duplicate tray icons ([539920c](https://github.com/TheChasman/CSpy/commit/539920c3824838270b8efd995c6aaeff6801ffb5))
* kill instances immediately before launch, not at script start ([7699fc7](https://github.com/TheChasman/CSpy/commit/7699fc73480f728a0993cde60babb1b2d0305500))
* kill stale processes and port before switching modes, auto-install cargo-watch ([057933d](https://github.com/TheChasman/CSpy/commit/057933d87c2a80439c84f8fdf744faa81c3ab8bd))
* make tray icon more visible with white outline and light grey hollow region ([04dc4fb](https://github.com/TheChasman/CSpy/commit/04dc4fb7576b09f11e413b5a1ccffb5b6aa3ad9e))
* popover positioning, usage normalisation, bump font size ([7f1ff40](https://github.com/TheChasman/CSpy/commit/7f1ff405f2cf60f4fc4dc4c64a3d4600b439b0a9))
* rename release-please config to drop erroneous dot prefix ([433eb3e](https://github.com/TheChasman/CSpy/commit/433eb3ea1ce0e3937176a0c1975aae85bad94d30))
* restore Vite background start in dev.sh ([59168ae](https://github.com/TheChasman/CSpy/commit/59168ae94e8a4c8495c4ad9cc4dd0daddc787ce7))
* simplify dev.sh — cargo tauri dev handles Rust watching, no launchd hot-swap in dev mode ([b0038f3](https://github.com/TheChasman/CSpy/commit/b0038f3f4041130169ad86fa6c568086d52f6896))
* stale token recovery, quiet hours, improved tray icon, launchd install ([444878c](https://github.com/TheChasman/CSpy/commit/444878c2707ffaede29cbc9465269593af890b79))
* use ditto --noextattr to strip provenance on app bundle install ([ea31f1d](https://github.com/TheChasman/CSpy/commit/ea31f1d06e5255a1728c40493cdf9b9fd4904bd2))
* use logical coordinates throughout for popover positioning ([16d984b](https://github.com/TheChasman/CSpy/commit/16d984b1a805128ec3cf3d45a5c93856dcbaa66a))
