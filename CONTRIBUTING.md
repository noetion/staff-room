# Contributing

The Staff Room currently supports Windows. Contributions that improve reliability, safety, usability, or documentation are welcome. For larger changes, please open an issue first so we can agree on the scope.

Before opening a pull request:

1. describe the user-visible problem and any safety boundary it affects;
2. keep the change scoped and preserve existing compatibility or document the break;
3. add focused tests for changed behavior;
4. run `npm run check` from a clean install;
5. record any packaged-app or live-provider observation that cannot be automated.

Please use synthetic data in fixtures, screenshots, and bug reports. Do not include private repositories, credentials, paid provider transcripts, or personal filesystem paths. Please report security issues through the process in [SECURITY.md](SECURITY.md), rather than the public issue tracker.
