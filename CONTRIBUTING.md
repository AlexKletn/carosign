# Contributing to CaroSign

[Русская версия](CONTRIBUTING.ru.md)

Follow the [code quality agreement](CODE_QUALITY.md) and write
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) in English.

Keep changes in the [part of CaroSign](README.md#repository-layout) that owns the behavior. Make changes to reusable dependencies and separate hardware providers in their own repositories. When CaroSign needs a new dependency revision, publish the dependency commit and update the pinned revision.

Run the checks for the component you changed. Cover changes to the service or client protocol with the mock provider where applicable. In the pull request, describe what you verified and identify the provider and hardware if you tested on a device.

Use synthetic values in shared examples and logs. Keep device-specific identifiers, biometric data, firmware dumps, and local notes out of commits.
