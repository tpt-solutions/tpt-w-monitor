# tpt-monitor user guide

tpt-monitor shows what is using your computer's CPU, memory, disk, network and GPU, and
it suggests what you might close or slow down. It never changes anything on its own.

## Install and first run

### Windows

1. Run the installer. It is not yet published; see the release notes when it is.
2. Start tpt-monitor. Windows asks for administrator rights. Accept them to see every
   process, its network traffic, and its GPU use, and to be able to change processes.
3. If you decline, the app still runs. A card on the dashboard says what is hidden. You
   can start it again from an elevated session later.

### Linux

1. Install the package for your distribution (not yet published).
2. To see every process, start tpt-monitor with `sudo tpt-monitor`. A desktop launcher that
   asks for your password through polkit is planned (see `docs/linux-elevation.md`).
3. Without root, the app still works, and the onboarding card says which processes are hidden.

## The dashboard

The dashboard is a grid of widgets. Drag a widget by its title bar to move it, and drag
its corner to resize it. Use **Add widget** to add one, and the × on a widget to remove it.

- **CPU:** total load over the last two minutes. Tick *Per-core* for each core's load.
- **Memory:** memory in use, with history.
- **Temperature & fans:** shown when the hardware exposes them. Otherwise the widget says
  they are unavailable. A guess is never shown.
- **Processes:** every process. Filter by name or PID, sort by any column, and tick
  *Group by name* to combine processes that share a program. Each row has a small CPU
  trend. Suspend, resume, change priority, or kill a process from its row.
- **Network:** total upload and download, and the processes using the most TCP traffic.
- **Disk I/O:** the processes doing the most reads and writes.
- **GPU:** the processes using the GPU, and their video memory.
- **Suggestions:** see below.

Layouts: use **Save layout** to keep the current arrangement under a name, and **Load** to
bring one back. The last session is saved automatically and restored when you start again.

## Suggestions

A suggestion appears when a background program (or any program while you are away from
the keyboard) keeps using a lot of CPU, or keeps reading and writing to disk, for most of
the last half minute. Each suggestion says why, and offers one action.

- **Apply** performs the offered action. It is always reversible. Suspend pauses the
  program; resume brings it back.
- **Don't suggest again** stops suggestions for that program. Use **Dismissed** to bring
  them back.

Suggestions never kill a program. To end a program, use Kill in the process list, which
asks you to confirm first.

Some programs are protected: the system's own programs, such as the login and display
services. The app never suggests them and refuses to kill or suspend them, even if you
ask. You can add your own names under **Protected processes**. You can remove only the
names you added.

## Desktop overlays

Use the ⧉ button on a widget to pop it out into its own window. Overlays stay on top of
other windows and do not appear in the taskbar.

- Drag the bar at the top to move the overlay, and drag its edges to resize it.
- The slider changes its opacity.
- **Click-through** lets mouse clicks pass through to whatever is underneath. Turn it off
  from the tray menu, because the overlay cannot be clicked while it is on.
- **Dock** returns the widget to the dashboard. **×** removes it.

Overlays are restored in the same place when you start the app again.

## The tray icon

The icon shows two small bar graphs: CPU on the left and memory on the right. Hover for
the exact numbers. The menu lists the three busiest processes and offers Open dashboard,
Toggle click-through for overlays, and Quit.

## When something shows "unavailable"

The app shows a value only when it can read it. A dash, or an "unavailable" message,
means the source is missing on this machine, or needs administrator rights, or is not
supported yet on this platform. A process that has no network traffic shows zero, but
only when the traffic counters could be read.

## Privacy

tpt-monitor makes no network connections. Layouts, overlay positions and your protected
names are stored only in the app's data folder on your computer.
