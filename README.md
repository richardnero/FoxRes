# FoxRes

A resistor calculator for GNOME, written in Rust. By [FoxIQ](https://www.foxiq.io).

- **Color code**: decode 3, 4, 5 and 6-band resistors, or type a value (`4k7`, `220R`, `1M`) and get its bands
- **SMD**: read and generate 3-digit, 4-digit, R-notation and EIA-96 markings
- **E-series**: nearest E3 to E192 standard value, with the values on either side and their error
- **Combine**: best series or parallel pair of standard values for a value you don't stock

## Layout

```
crates/foxres-core   resistor math, no GUI dependencies, fully unit-tested
crates/foxres-gtk    GTK 4 + libadwaita front end (Relm4)
data/                desktop file, AppStream metainfo, icon
snap/                snapcraft.yaml
```

`foxres-core` builds on any platform with nothing but Rust installed:

```sh
cargo test -p foxres-core
```

## Building the app

Ubuntu 24.04 or newer:

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev
cargo run --release
```

The app targets the GNOME 46 API (GTK 4.14, libadwaita 1.5) so it runs in the
`core24` snap runtime.

## Snap

```sh
sudo snap install snapcraft --classic
snapcraft pack
sudo snap install --dangerous ./foxres_*.snap
```

CI also builds the snap on every push to `main`; download it from the run's artifacts.

## License

MIT. See [LICENSE](LICENSE).
