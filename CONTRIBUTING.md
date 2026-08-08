# Contributing

The Staff Room is deliberately small and Windows-first. Contributions should strengthen the current human-gated product rather than broaden its platform or provider surface without a demonstrated need.

Before opening a pull request:

1. describe the user-visible problem and the custody boundary it affects;
2. keep the change scoped and preserve existing compatibility or document the break;
3. add focused tests for changed behavior;
4. run `npm run check` from a clean install;
5. record any packaged-app or live-provider observation that cannot be automated.

Never use a real private repository, credential, paid provider transcript, or personal filesystem path in a fixture, screenshot, or bug report. Security issues follow [SECURITY.md](SECURITY.md), not the public issue tracker.
