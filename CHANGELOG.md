# Changelog

## [0.2.1](https://github.com/Project-Colony/Exospine/compare/v0.2.0...v0.2.1) (2026-10-14)


### Fixes

* **accounts:** add Outlook accounts through OAuth and keep the keyring in step with the account list ([#16](https://github.com/Project-Colony/Exospine/issues/16)) ([4299b71](https://github.com/Project-Colony/Exospine/commit/4299b71737696bcc4b8d252d6f12c82a97ca0181))
* **accounts:** reject non-address Outlook usernames and finish secure wipe after a failed delete ([#17](https://github.com/Project-Colony/Exospine/issues/17)) ([ef2de03](https://github.com/Project-Colony/Exospine/commit/ef2de03e1ffc47584a4b848ead40f6a686b1d066))
* **deps:** update ammonia to 4.2.1 (GHSA-m6mh-2hw2-555x) ([#2](https://github.com/Project-Colony/Exospine/issues/2)) ([5ba6665](https://github.com/Project-Colony/Exospine/commit/5ba66654c3899f6b513d250a76bb39bd1401740d))
* **deps:** update tauri, openssl, rustls and other vulnerable crates ([#4](https://github.com/Project-Colony/Exospine/issues/4)) ([42e6ae1](https://github.com/Project-Colony/Exospine/commit/42e6ae11c16be45f6bc5ebe60a87b3d5fa1015fc))
* open the mail database on a fresh install ([#1](https://github.com/Project-Colony/Exospine/issues/1)) ([341e331](https://github.com/Project-Colony/Exospine/commit/341e3315ec57525529e57d0797c25526d56df76b))
* **security:** keep credentials in the OS keyring and out of the webview ([#14](https://github.com/Project-Colony/Exospine/issues/14)) ([053830c](https://github.com/Project-Colony/Exospine/commit/053830cc56a95da752de1fdc362275480ab97106))
* **security:** sanitize email HTML before it reaches any window ([#13](https://github.com/Project-Colony/Exospine/issues/13)) ([7c6cef9](https://github.com/Project-Colony/Exospine/commit/7c6cef978347fe2ee5b23adee144f6188ce9acf5))
