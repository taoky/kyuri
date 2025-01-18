# kyuri

A progress display library specifically designed for my mirroring softwares ([tsumugu](https://github.com/taoky/tsumugu) & [yukina](https://github.com/taoky/yukina)).

Uses some code from [indicatif](https://github.com/console-rs/indicatif) (MIT).

Docs: [docs.rs/kyuri](https://docs.rs/kyuri)

The API is not stable yet.

## Why?

It could output a progress indicator both when printing to terminal and (especially when) writing to file, with minimal distraction.

And no other dependencies.

If you need a progress bar or spinner with rich and fancy features, use [indicatif](https://github.com/console-rs/indicatif) instead.

## Examples

- [Simple example](examples/progress.rs)

    <img src="https://github.com/taoky/kyuri/blob/master/assets/progress.gif?raw=true" alt="progress">

    ```shell
    cargo run --example progress
    cargo run --example progress > file
    ```

- 2 threads example ([thread](examples/thread.rs), [download](examples/download.rs))

    <img src="https://github.com/taoky/kyuri/blob/master/assets/thread.gif?raw=true" alt="thread">

    ```shell
    cargo run --example thread
    cargo run --example thread > file
    ```

    or this with a different pattern:

    <img src="https://github.com/taoky/kyuri/blob/master/assets/download.gif?raw=true" alt="download">

    ```shell
    cargo run --example download
    cargo run --example download > file
    ```

- Progress bar example

    <img src="https://github.com/taoky/kyuri/blob/master/assets/bar.gif?raw=true" alt="bar">

    ```shell
    cargo run --example bar
    cargo run --example bar > file
    ```

- `tracing` integration example

    <img src="https://github.com/taoky/kyuri/blob/master/assets/tracing.gif?raw=true" alt="tracing">

    ```shell
    cargo run --example tracing
    cargo run --example tracing > file
    ```

---

<img src="https://github.com/taoky/kyuri/blob/master/assets/cucumber.jpg?raw=true" alt="Cucumber from Mutsumi">

(Taken from *[BanG Dream! It's MyGO!!!!!](https://en.wikipedia.org/wiki/MyGO!!!!!)* episode 6.)
