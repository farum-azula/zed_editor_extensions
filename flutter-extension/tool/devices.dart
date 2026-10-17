// Turns a run target (`ios-simulator`, `android`, `web`, `macos`, `windows`, `linux`) into the id of
// a device `flutter run -d` can use, starting a simulator or an emulator when none is running.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// The run targets, in the order they are offered, with their names.
const targets = {
  'ios-simulator': 'iOS Simulator',
  'android': 'Android',
  'web': 'Web',
  'macos': 'macOS',
  'windows': 'Windows',
  'linux': 'Linux',
};

class DeviceException implements Exception {
  DeviceException(this.message);

  final String message;

  @override
  String toString() => message;
}

/// How often the device list is read again while a simulator or an emulator boots.
Duration get pollInterval => Duration(
  milliseconds:
      int.tryParse(Platform.environment['FLUTTER_TOOLS_POLL_MS'] ?? '') ?? 2000,
);

/// How long a simulator or an emulator may take to boot.
const bootTimeout = Duration(minutes: 5);

/// The device id for [target]. [flutter] is the command that runs Flutter (`flutter`, or `fvm
/// flutter`); [log] reports what is happening, a line at a time.
Future<String> resolveDevice(
  String target,
  List<String> flutter,
  void Function(String) log,
) async {
  switch (target) {
    case 'macos' || 'windows' || 'linux':
      final host = Platform.operatingSystem;
      if (host != target) {
        throw DeviceException(
          '${targets[target]} apps run only on $target; this is $host.',
        );
      }
      return target;
    case 'web':
      final devices = await listDevices(flutter);
      for (final id in ['chrome', 'edge']) {
        if (devices.any((d) => d.id == id)) {
          return id;
        }
      }
      log(
        'No Chrome or Edge found: serving the app on web-server; open the URL it prints.',
      );
      return 'web-server';
    case 'ios-simulator':
      final simulator = await _deviceOrBoot(
        flutter,
        log,
        (d) => d.platform == 'ios' && d.emulator,
        'an iOS Simulator',
        () => _bootSimulator(log),
      );
      // A simulator can be booted with no window, Simulator.app closed: the app would run where
      // nobody sees it. Opening Simulator.app on it shows its window, and is harmless when it is
      // showing already.
      await _showSimulator(simulator);
      return simulator;
    case 'android':
      return _deviceOrBoot(
        flutter,
        log,
        (d) => d.platform.startsWith('android'),
        'an Android device or emulator',
        () => _launchEmulator(flutter, log),
      );
    default:
      throw DeviceException(
        'unknown target "$target"; use one of ${targets.keys.join(', ')}',
      );
  }
}

class Device {
  Device(this.id, this.name, this.platform, this.emulator);

  final String id;
  final String name;
  final String platform;
  final bool emulator;
}

Future<List<Device>> listDevices(List<String> flutter) async {
  final result = await runFlutter(flutter, ['devices', '--machine']);
  if (result.exitCode != 0) {
    throw DeviceException('`flutter devices` failed: ${result.stderr}');
  }
  final out = result.stdout as String;
  final start = out.indexOf('[');
  if (start < 0) {
    return [];
  }
  final list = jsonDecode(out.substring(start)) as List;
  return [
    for (final d in list.cast<Map<String, Object?>>())
      Device(
        d['id'] as String,
        d['name'] as String? ?? '',
        d['targetPlatform'] as String? ?? '',
        d['emulator'] == true,
      ),
  ];
}

Future<ProcessResult> runFlutter(List<String> flutter, List<String> args) =>
    Process.run(flutter.first, [
      ...flutter.skip(1),
      ...args,
    ], runInShell: Platform.isWindows);

Future<String> _deviceOrBoot(
  List<String> flutter,
  void Function(String) log,
  bool Function(Device) wanted,
  String what,
  Future<void> Function() boot,
) async {
  final running = (await listDevices(flutter)).where(wanted);
  if (running.isNotEmpty) {
    return running.first.id;
  }
  await boot();
  final deadline = DateTime.now().add(bootTimeout);
  while (DateTime.now().isBefore(deadline)) {
    await Future<void>.delayed(pollInterval);
    final booted = (await listDevices(flutter)).where(wanted);
    if (booted.isNotEmpty) {
      log('${booted.first.name} is ready.');
      return booted.first.id;
    }
  }
  throw DeviceException(
    '$what did not come up within ${bootTimeout.inMinutes} minutes.',
  );
}

/// Boots an iPhone simulator of the newest iOS runtime and opens the Simulator app.
Future<void> _bootSimulator(void Function(String) log) async {
  if (!Platform.isMacOS) {
    throw DeviceException('the iOS Simulator runs only on macOS.');
  }
  final list = await Process.run('xcrun', [
    'simctl',
    'list',
    'devices',
    'available',
    '-j',
  ]);
  if (list.exitCode != 0) {
    throw DeviceException(
      '`xcrun simctl` failed; is Xcode installed? ${list.stderr}',
    );
  }
  final runtimes = (jsonDecode(list.stdout as String)['devices'] as Map)
      .cast<String, List>();
  final ios = runtimes.keys.where((r) => r.contains('.iOS-')).toList()
    ..sort(_compareRuntimes);
  for (final runtime in ios.reversed) {
    final phones = runtimes[runtime]!.cast<Map>().where(
      (d) => '${d['name']}'.startsWith('iPhone'),
    );
    if (phones.isEmpty) {
      continue;
    }
    final phone = phones.first;
    log('Starting the iOS Simulator (${phone['name']})…');
    await Process.run('xcrun', ['simctl', 'boot', '${phone['udid']}']);
    return;
  }
  throw DeviceException(
    'no iPhone simulator is installed; add one in Xcode → Settings → Components.',
  );
}

/// Opens Simulator.app on the simulator [udid]. `FLUTTER_TOOLS_OPEN_SIMULATOR` (a JSON list) is the
/// command to run instead, given the udid, for the tests.
Future<void> _showSimulator(String udid) async {
  final override = Platform.environment['FLUTTER_TOOLS_OPEN_SIMULATOR'];
  final command = override != null
      ? [...(jsonDecode(override) as List).cast<String>(), udid]
      : ['open', '-a', 'Simulator', '--args', '-CurrentDeviceUDID', udid];
  await Process.run(command.first, command.skip(1).toList());
}

/// `...SimRuntime.iOS-18-2` before `...SimRuntime.iOS-26-0`.
int _compareRuntimes(String a, String b) {
  List<int> version(String r) => r
      .split('.iOS-')
      .last
      .split('-')
      .map((p) => int.tryParse(p) ?? 0)
      .toList();
  final va = version(a), vb = version(b);
  for (var i = 0; i < va.length && i < vb.length; i++) {
    if (va[i] != vb[i]) {
      return va[i].compareTo(vb[i]);
    }
  }
  return va.length.compareTo(vb.length);
}

/// Launches the first Android emulator `flutter emulators` lists.
Future<void> _launchEmulator(
  List<String> flutter,
  void Function(String) log,
) async {
  final result = await runFlutter(flutter, ['emulators']);
  final emulators = [
    for (final line in LineSplitter.split(result.stdout as String))
      if (line.contains('•') && line.trim().endsWith('android'))
        line.split('•').map((p) => p.trim()).toList(),
  ];
  if (emulators.isEmpty) {
    throw DeviceException(
      'no Android device is connected and no emulator is set up; '
      'create one with `flutter emulators --create` or in Android Studio.',
    );
  }
  log('Starting the Android emulator ${emulators.first[1]}…');
  final launch = await runFlutter(flutter, [
    'emulators',
    '--launch',
    emulators.first[0],
  ]);
  if (launch.exitCode != 0) {
    throw DeviceException(
      '`flutter emulators --launch` failed: ${launch.stdout}${launch.stderr}',
    );
  }
}

/// The command that runs Flutter for the project in [dir]: `fvm flutter` when the project pins a
/// Flutter version with fvm, `flutter` otherwise. `FLUTTER_TOOLS_FLUTTER` (a JSON list) overrides it.
List<String> flutterCommand(Directory dir) {
  final override = Platform.environment['FLUTTER_TOOLS_FLUTTER'];
  if (override != null) {
    return (jsonDecode(override) as List).cast<String>();
  }
  final fvm =
      File('${dir.path}/.fvmrc').existsSync() ||
      Directory('${dir.path}/.fvm').existsSync();
  return fvm ? ['fvm', 'flutter'] : ['flutter'];
}
