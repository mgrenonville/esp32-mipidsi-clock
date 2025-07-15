# ESP32-MIPI-DSI Pokémon Clock

A WiFi-connected **Pokémon-themed clock** powered by an **ESP32** microcontroller and an MIPI DSI LCD display. 
This project gives new life to a nostalgic children's Pokémon clock by replacing its original electronics with modern, 
programmable hardware. It features animated Pokémon graphics, time synchronization with NTP, and is fully customizable.

➡️ Read the full blog post: [Brain Transplant of a Dumb Pokémon Clock](http://www.blogouillage.net/2025/07/brain-transplant-of-dumb-pokemon-clock.html)


## Table of Contents

- [Project Overview](#project-overview)
- [Features](#features)
- [Hardware Requirements](#hardware-requirements)
- [Software Requirements](#software-requirements)
- [Installation](#installation)
- [Building](#building)
- [Running](#running)
- [Acknowledgments](#acknowledgments)
- [License](#license)

## Project Overview

This project transforms a toy Pokémon alarm clock into a modern IoT device using an **ESP32-C6** and a **MIPI DSI LCD screen**. 
It's a fun and hackable hardware/software project that combines nostalgia with embedded graphics, 
powered by the [slint-ui](https://slint.dev/) graphics library.

## Features

- 💡 Animated Pokémon clock face
- 🦀🐚 nostd ! True rust experience
- 🖥️ Desktop simulator, to speedup development
- 🖼️ Display via ST7789 compatible display (through [mipidsi](https://github.com/almindor/mipidsi) project)
- 🌐 WiFi connectivity
- 🕒 NTP time synchronization & Realtime clock DS3231
- 🔧 Easily customizable and open-source
- ⚡️ Embassy framework
- 🎮 Hackable


## Hardware Requirements

- An [ESP32-C6 microcontroller](https://www.aliexpress.com/item/1005007219898210.html), but every ESP32 microcontroller should work, after adaptations
- A [240x240 Round LCD display](https://www.aliexpress.com/item/1005005925857858.html) (GC9A01)
- A [realtime clock module](https://www.aliexpress.com/item/1005007143542894.html) (here a DS3231, but a DS1307 works flawlessly with the right lib)
- A [Repurposed Pokémon clock frame](https://www.amazon.fr/TEKNOFUN-Pokemon-r%C3%A9veil-Salam%C3%A8che-811368/dp/B075YYKBX4) (optional)


## Software Requirements

- Rust 1.85
- Follow instructions of [Rust on ESP book, RISC-V, nostd](https://docs.espressif.com/projects/rust/book/installation/riscv.html)
- Slint-UI 1.12

## Installation

1. **Clone the Repository**
```
git clone https://github.com/mgrenonville/esp32-mipidsi-clock.git
cd esp32-mipidsi-clock
```

2. **Build & run the simulator**
```
cargo run --bin ui_simulator --no-default-features --features=simulator --target x86_64-unknown-linux-gnu
```
See `src/bin/ui_simulator.rs` for the key bindings


## Building
Using ![ESP32-C6 module pins definition](/pins-def-esp32c6.png "ESP32-C6 Pin definitions") 
and following table, wire the board. 
| PIN    | Function       | Description     | Notes          |
|--------|----------------|-----------------|----------------|
| GPIO0  | keyboard       | S1              |                |
| GPIO1  | keyboard       | S2              |                |
| GPIO2  | keyboard       | S3              |                |
| GPIO3  | Screen         | Reset           |                |
| GPIO4  | Screen         | CS              |                |
| GPIO5  | Screen         | Backlight       |                |
| GPIO6  | I2C - DS3231   | SCL             |                |
| GPIO7  | I2C - DS3231   | SDA             |                |
| GPIO8  | RGB LED        |                 | future work    |
| GPIO9  | keyboard       | Common          |                |
| GPIO12 |                |                 |                |
| GPIO13 |                |                 |                |
| GPIO14 |                |                 |                |
| GPIO15 | Screen         | DC              |                |
| GPIO18 | Screen         | SCK             |                |
| GPIO19 | Screen         | MOSI            |                |
| GPIO20 |                |                 |                |
| GPIO21 |                |                 |                |
| GPIO22 |                |                 |                |
| GPIO23 |                |                 |                |

## Running
Create a file `.env` based on `.env.template` with your timezone, SSID and WIFI passord, 
plug the board and execute: 
``` 
cargo espflash flash --release --monitor
```

## Acknowledgments
- Huge thanks to Warren Clark / Woostar Pixels ([Portfolio](https://www.artstation.com/woostarpixels)) for allowing me to use his artwork
- Kudos to Embassy, expressif, rust, and slint-ui project, for these amazing tools

## License
This project is licensed under the [MIT License](LICENSE).

