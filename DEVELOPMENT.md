
# Radicle Surfing 🏄

Thanks for wanting to contribute to `radicle-surf`!

# Licensing

We are [GPL-3.0-or-later](./LICENSE) licensed project. To keep in compliance with this we must
add a [license header](./.license-header) to any new files added. This is checked on each run of CI.

## Building & Testing 🏗️

We try to make development as seemless as possible so we can get down to the real work. We supply
the toolchain via the `rust-toolchain` file, and the formatting rules `.rustmt.toml` file.

For the [Nix](https://nixos.org/) inclined there is a `default.nix` file to get all the necessary
dependencies and it also uses the `rust-toolchain` file to pin to that version of Rust.

The `build.rs` file takes care of setting up the submodule `data/git-platinum`. So to build and test
`radicle-surf` all that is necessary is running:

```bash
$ cargo build
```

and

```bash
$ cargo test
```

For the full list of commands that get executed on CI you can checkout the [ci/run](./ci/run) script.

If any of this _isn't_ working, then let's work through it together and get it Working on Your
Machine™.

## Structure 🏛️

The design of `radicle-surf` is to have an in-memory representation of a project's directory which
can be generated by a VCS's backend. The directory system is modeled under `file_system`, the VCS
functionality is naturally under `vcs`, and `diff` logic is held under `diff`.

```
src/
├── diff
├── file_system
└── vcs
```

## Testing & Documentation 📚

We ensure that the crate is well documented. `cargo clippy` will argue with you anytime a public
facing piece of the library is undocumented. We should always provide an explanation of what
something is or does, and also provide examples to allow our users to get up and running as quick
and easy as possible.

When writing documentation we should try provide one or two examples (if they make sense). This
provides us with some simple unit tests as well as something our users can copy and paste for ease
of development.

If more tests are needed then we should add them under `mod tests` in the relevant module. We strive
to find properties of our programs so that we can use tools like `proptest` to extensively prove our
programs are correct. As well as this, we add unit tests to esnure the examples in our heads are
correct, and testing out the ergonomics of our API first-hand.

## CI files 🤖

Our CI infrastructure runs on Buildkite. The build process is run for every commit which is pushed
to GitHub.

All relevant configuration can be found here:

```
radicle-surf/.buildkite/
├── docker
│   ├── build
│   │   └── Dockerfile
│   └── rust-nightly
│       └── Dockerfile
└── pipeline.yaml
```

## Releases 📅

TODO: Once we get the API into a good shape we will keep track of releases via a `CHANGELOG.md` and
tag the releases via `git tag`.