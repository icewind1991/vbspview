# vbspview

tf2 map viewer based on [vbsp](https://github.com/icewind1991/vbsp)

## Usage

```
cargo run --release -- /path/to/map.bsp
```

Note that asset loading isn't very well optimized so loading maps can take a while.

In order to load the assets referenced by the map, TF2 needs to be installed locally.

![pl_badwater as rendered by the viewer](screenshots/badwater.png)