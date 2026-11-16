# KeyGlow

**Keystroke *visualization* engine for Orbit — it never sees what you type.**

> Orbit companion module — phase 2/4 of the [Orbit ecosystem](https://github.com/Can-5/orbit).
> Standalone crate: usable without Orbit, without other modules, and without network.

## Implemented now

- TypingStats: sliding-window WPM from raw key events (codes only, never characters)
- KeyProfile / AnimationKind / SoundProfile models; positional activation rules
- serde event models used by tests (synthetic events — no real hooks)

## Not implemented yet (explicitly out of scope for this crate)

- No global key hook — that requires the KeyboardListen permission and lives in the Orbit host (evdev/X11/Win32)
- No rendering, no sound playback

## Build & test

```sh
cargo test  # 7 tests
```

Dependencies: serde, thiserror  # serde_json (dev). No cross-repo path dependencies, no Git dependencies, no downloads at build time beyond crates.io.

## Privacy

KeyGlow never receives the characters you type — only key identity and timing. Stats stay on device; there is no network code.

## License

MIT © The Orbit contributors. See [LICENSE](./LICENSE).
