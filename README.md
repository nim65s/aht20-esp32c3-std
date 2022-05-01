# aht20 sensor on esp32-c3, connect to wifi and publish on MQTT

```bash
cargo espflash --release --monitor /dev/ttyUSB0  # adapt your tty here
```

Basically https://github.com/esp-rs/esp-idf-template + https://github.com/fmckeogh/aht20 + tasmota mqtt style

no-std version: https://github.com/nim65s/aht20-esp32c3-nostd
