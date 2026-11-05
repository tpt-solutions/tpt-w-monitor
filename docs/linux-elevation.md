# Linux: running with root rights

tpt-monitor needs root to see and control every process. There are three ways to run it
with root, from the simplest to the most integrated.

## 1. From a terminal with sudo

```sh
sudo tpt-monitor
```

This is the default and needs nothing installed. The onboarding card reports that the run
is elevated.

## 2. From a desktop launcher, with a polkit prompt

The planned desktop entry will launch the app through `pkexec`, and polkit will ask for your
password through the desktop's own dialog. It is not packaged yet. The action is defined in
`packaging/linux/com.tptsolutions.tptmonitor.policy`:

- The action id is `com.tptsolutions.tptmonitor.run`.
- It requires administrator authentication (`auth_admin`) for active sessions, and never
  allows it for inactive or remote sessions.
- It runs only the installed binary, `/usr/bin/tpt-monitor`, so the policy cannot be used to
  run anything else.

To install the policy by hand:

```sh
sudo install -m 644 packaging/linux/com.tptsolutions.tptmonitor.policy /usr/share/polkit-1/actions/
```

## 3. Without root

The app still runs. Processes owned by other users are hidden, and actions on them are
refused. The onboarding card lists what is hidden.

## Checking the policy file

```sh
xmllint --noout packaging/linux/com.tptsolutions.tptmonitor.policy
```

Not yet run on a real distribution: see `todo.md`, Phase 3, for the distro checks.
