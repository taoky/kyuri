# kyuri

A progress display library specifically designed for my mirroring softwares (tsumugu & yukina).

Uses some code from [indicatif](https://github.com/console-rs/indicatif) (MIT).

## Why?

It could output a progress indicator both when printing to terminal and (especially when) writing to file, with minimal distraction.

And no other dependencies.

If you need a progress bar or spinner with rich and fancy features, use [indicatif](https://github.com/console-rs/indicatif) instead.

## Examples

```shell
cargo run --example thread
cargo run --example thread > file
```
