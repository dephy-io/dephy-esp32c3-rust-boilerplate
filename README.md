dephy-esp32c3-rust-boilerplate
---
Build DePHY application on ESP32C3 with `esp-idf` and Rust!

This boilderplate brings you:
- [x] Storing `secp256k1` private key in eFuse
- [x] DePHY message creating/verifying
- [x] Send DePHY messages via HTTP(S)
- [ ] Send/subscribe DePHY messages via MQTT

And also:
- [x] Rust `std` support
- [x] Cargo-based toolchain (which means no CMake needed in your project)
- [x] Basic logging
- [x] Basic Wi-Fi/BLE connection
- [x] Wi-Fi provisioning with BLE
- [x] GPIO example
- [x] I2C example with Mysentech M117B temperature sensor
- [x] HTTP/HTTPS access example
- [x] protobuf usage example
- [x] `async` support with `tokio`

### How to build

1. [Install Rust following the official document](https://doc.rust-lang.org/book/ch01-01-installation.html).
2. Install dependencies and tools:
```shell
# macOS
brew install libuv
# Debian/Ubuntu/etc.
apt-get install libuv-dev
# Fedora
dnf install systemd-devel

rustup toolchain install nightly --component rust-src
rustup target add riscv32imc-unknown-none-elf

cargo install ldproxy
cargo install espup
cargo install espflash
cargo install cargo-espflash
```
3. Create a `build.env` and apply some configurations on it:
```shell
cp example.build.env build.env

# or just create the file
touch build.env
```
4. Build and flash:
```bash
cargo run

# equals to
cargo espflash flash --monitor --partition-table huge_app.csv
```

### Booting Behavior

1. The firmware checks if keys are burnt in eFuse, if no, it enters `Key Inspect Mode`:
   1. it checks if keys are burnt in eFuse, if yes, jump to `v.`;
   2. it starts Wi-Fi and BLE modem for collecting entropy for hardware RNG;
   3. it waits for about 1 hour before generate the key, during this, the 2 LEDs will blink alternately;
   4. it generates a random private key from the hardware RNG and writes it to eFuse;
   5. it prints `device name with MAC address`, `public key`, and the corresponding `ethereum address` to the serial console every 10 seconds, during this, the 2 LEDs will blink simultaneously.
 

3. The firmware checks if the Wi-Fi should be provisioned, if no, it enters `Wi-Fi Provisioning Mode`:
   - it uses the [Unified Provisioning](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/provisioning/provisioning.html) protocol provided by the `esp-idf` SDK;
   - official provisioning app provided by Espressif are available for iOS([App Store](https://apps.apple.com/in/app/esp-ble-provisioning/id1473590141), [Source](https://github.com/espressif/esp-idf-provisioning-ios)) and Android([Google Play](https://play.google.com/store/apps/details?id=com.espressif.provble), [APK](https://github.com/espressif/esp-idf-provisioning-android/releases), [Source](https://github.com/espressif/esp-idf-provisioning-android)).
   - the firmware will start the provisioning session in `BLE mode` with `Security 1 Scheme`, the `pop` parameter is set to `abcd1234`(default value in official provisioning Apps) for convenient testing;
   - the 2 LEDs will blink alternately and rapidly during the provisioning session.


3. If keys and Wi-Fi are well provisioned, the firmware waits for button input for boot modes:
   - during waiting, the 2 LEDs will blink simultaneously and rapidly;
   - press the button for 2-6 seconds then release it, the firmware enters `Wi-Fi Provisioning Mode`(referring to `2.`);
   - press the button for more than 12 seconds, the firmware enters `Key Inspect Mode`(referring to `1.`);
   - if there had been no input for 12 seconds, the firmware starts the app.




### Build Configurations
Build configurations are stored in `build.env` and only being read on building.

| Key                        | Rust Type | Comment                                                                                                     |
|----------------------------|-----------|-------------------------------------------------------------------------------------------------------------|
| `BUILD_PRINT_EXPANDED_ENV` | `bool`    | Weather to print generated codes in `cargo run`. Default to be `false`.                                     |
| `DEPHY_ENDPOINT_HTTP`      | `&str`    | The endpoint to publish DePHY messages. Default to be `https://send.testnet.dephy.io/dephy/signed_message`. |
| `APP_SEND_LOOP_DURATION`   | `u64`     | Time duration of one cycle in the send loop in seconds. Default to be `10`.                                 |
