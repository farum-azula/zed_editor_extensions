# Flutter Tools

Flutter and Dart snippets for [Zed](https://zed.dev): 141 of them, for widgets, layouts, scrolling,
builders, navigation, dialogs, forms, animations, Dart declarations and widget tests. Plain Flutter
and Dart only, no packages.

Type a prefix and pick the snippet from the completion menu. Prefixes are short, mostly the first
letters of the words: `sfp` is a **s**tate**f**ul widget with a **p**arameter, `lvb` a
**L**ist**V**iew.**b**uilder, `ftb` a **F**u**t**ure**B**uilder. **Tab** moves to the next field; a
field that appears more than once (a class name, say) is edited everywhere at once; fields with a
fixed set of values (`MainAxisAlignment.start`, `BoxFit.cover`, ...) offer them as a list. Every
field has a default, and with the defaults the code compiles once the names it refers to
(`items`, `_controller`, ...) exist.

The Dart language server has snippets of its own, so these do not repeat them: `stless`, `stful`,
`stanim`, `class`, `main`, `fun`, `for`, `forin`, `if`, `ife`, `switch`, `try`, `while`, `do`,
`test`, `group`.

## Installation

Run `install.sh` (macOS, Linux) or `install.ps1` (Windows) from the repository root, or command
palette → `zed: install dev extension` → pick the `flutter-extension` folder. See the
[repository README](../README.md).

## Snippets

### App

| Prefix | Inserts |
|---|---|
| `mapp` | A MaterialApp with a home screen |
| `capp` | A CupertinoApp with a home screen |
| `mna` | main() that initializes bindings before runApp |
| `thd` | A Material 3 ThemeData from a seed color |

### Widget classes

| Prefix | Inserts |
|---|---|
| `slc` | A StatelessWidget that wraps a child |
| `sfp` | A StatefulWidget with a field read through widget. |
| `inh` | An InheritedWidget with of() and maybeOf() |
| `cn` | A ChangeNotifier with one notifying field |
| `cpt` | A CustomPainter |
| `ccl` | A CustomClipper&lt;Path&gt; |

### State members

| Prefix | Inserts |
|---|---|
| `ins` | Override initState |
| `ds` | Override dispose |
| `dcd` | Override didChangeDependencies |
| `duw` | Override didUpdateWidget |
| `dea` | Override deactivate |
| `bld` | Override build |
| `ac` | An AnimationController (the State needs a TickerProvider mixin; dispose it) |
| `tc` | A TextEditingController (dispose it) |
| `sc` | A ScrollController (dispose it) |
| `fn` | A FocusNode (dispose it) |
| `fk` | A GlobalKey&lt;FormState&gt; |
| `gt` | A getter |
| `st` | A setter |

### Statements

| Prefix | Inserts |
|---|---|
| `ss` | Call setState |
| `ifm` | Stop when the State is no longer mounted (after an await) |
| `ifcm` | Stop when the BuildContext is no longer mounted (after an await) |
| `np` | Push a MaterialPageRoute |
| `npn` | Push a named route |
| `npr` | Replace the current route |
| `npa` | Push a route and remove all the others |
| `npo` | Pop the current route |
| `npu` | Pop back to the first route |
| `sd` | Show an AlertDialog with cancel and confirm actions |
| `sbs` | Show a modal bottom sheet |
| `ssb` | Show a SnackBar through the ScaffoldMessenger |
| `pfc` | Run code after the current frame |
| `fd` | Wait for a duration |
| `tm` | Start a Timer (import dart:async) |
| `tmp` | Start a periodic Timer (import dart:async) |
| `cpl` | Create a Completer (import dart:async) |
| `tcf` | try with catch and finally |
| `dp` | Print in debug builds |
| `lg` | Log through dart:developer |
| `uf` | Dismiss the keyboard |
| `ifc` | Match a pattern |
| `fid` | Loop over a list with its indexes |

### Layout

| Prefix | Inserts |
|---|---|
| `scf` | A Scaffold with an AppBar |
| `ab` | An AppBar with actions |
| `col` | A Column |
| `row` | A Row |
| `stk` | A Stack |
| `pos` | A Positioned child of a Stack |
| `cen` | A Center |
| `pd` | Padding on all sides |
| `pds` | Horizontal and vertical padding |
| `sbh` | Vertical space |
| `sbw` | Horizontal space |
| `sbx` | A SizedBox with a child |
| `ct` | A Container |
| `dct` | A Container with a rounded BoxDecoration |
| `ex` | An Expanded |
| `fx` | A Flexible |
| `al` | An Align |
| `wr` | A Wrap |
| `sa` | A SafeArea |
| `crr` | Clip with rounded corners |
| `cd` | A Card with padding |
| `lt` | A ListTile |
| `dv` | A Divider |
| `cpi` | A centered progress indicator |
| `vis` | Show a child conditionally |

### Scrolling

| Prefix | Inserts |
|---|---|
| `lv` | A ListView of children |
| `lvb` | A lazily built ListView |
| `lvs` | A ListView with separators |
| `gvb` | A lazily built grid with a fixed column count |
| `gvc` | A grid of children with a fixed column count |
| `scv` | A scrollable Column |
| `csv` | A CustomScrollView with an app bar and a list |
| `slb` | A lazily built sliver list |
| `sla` | A box widget inside a CustomScrollView |
| `slp` | Padding around a sliver |
| `ri` | Pull to refresh |
| `pvb` | A lazily built PageView |
| `dtc` | A DefaultTabController with a TabBar and a TabBarView |

### Builders

| Prefix | Inserts |
|---|---|
| `bd` | A Builder for a new BuildContext |
| `lb` | Build from the parent constraints |
| `ftb` | Build from a Future, with loading and error states |
| `stb` | Build from a Stream, with loading and error states |
| `vlb` | Rebuild when a ValueListenable changes |
| `lnb` | Rebuild when a Listenable notifies |
| `anb` | Rebuild on every animation tick |

### Content

| Prefix | Inserts |
|---|---|
| `tx` | A Text with a theme style |
| `rt` | Text with several styles |
| `ic` | An Icon |
| `img` | An image from the assets |
| `imgn` | An image from the network, with loading and error states |
| `hr` | A Hero animation |

### Input

| Prefix | Inserts |
|---|---|
| `gd` | Detect taps |
| `iw` | A tappable area with a ripple |
| `eb` | An ElevatedButton |
| `flb` | A FilledButton |
| `ob` | An OutlinedButton |
| `tb` | A TextButton |
| `ib` | An IconButton |
| `fab` | A FloatingActionButton |
| `tf` | A TextField with a controller |
| `tff` | A TextFormField with a validator |
| `frm` | A Form with a submit button |
| `cbt` | A checkbox with a label |
| `swt` | A switch with a label |
| `sl` | A Slider |
| `dd` | A DropdownButton |

### Animation

| Prefix | Inserts |
|---|---|
| `anc` | A Container that animates its changes |
| `ano` | Fade a child in and out |
| `ans` | Animate between children |

### Theme and screen

| Prefix | Inserts |
|---|---|
| `mqs` | The size of the screen |
| `th` | The current theme |
| `tt` | The text theme |
| `cs` | The color scheme |

### Dart

| Prefix | Inserts |
|---|---|
| `imm` | Import package:flutter/material.dart |
| `imc` | Import package:flutter/cupertino.dart |
| `imw` | Import package:flutter/widgets.dart |
| `ims` | Import package:flutter/services.dart |
| `ima` | Import dart:async |
| `mdl` | An immutable class with copyWith, JSON, equality and toString |
| `en` | An enum with a field and a constructor |
| `sld` | A sealed class with two subclasses |
| `ext` | An extension |
| `mx` | A mixin |
| `sgl` | A class with a single instance |
| `td` | A type alias |
| `fa` | A function returning a Future |
| `fs` | A function generating a Stream |
| `todo` | A TODO comment |

### Tests

| Prefix | Inserts |
|---|---|
| `tw` | A widget test |
| `su` | Run before each test |
| `tdn` | Run after each test |
| `sua` | Run once before all tests |
| `tp` | Tap a widget and settle |
| `et` | Type into a text field |
| `ef` | Expect a widget to be found |

## Development

Snippets live in `snippets/dart.json`, in Zed's snippet syntax: `$1`, `${1:default}`,
`${1|one,two|}`, `$0` for the final cursor position. A `$` that does not start a field must be
written `\\$` in the JSON (`'\\$name'` for Dart's `'$name'`): Zed rejects the whole snippet
otherwise. Inside a field's default, a `}` is written `\\}`.

`tests/check_snippets.py` checks every snippet:

1. its prefix is its own, and not one of the Dart language server's;
2. Zed can parse it;
3. each repeated field has the same default everywhere;
4. expanded with its defaults and put where it is typed, it is Dart without errors (`flutter
   analyze` on a scratch Flutter project).

Where a snippet is typed (top level, a `State` member, a statement, a widget, an expression, a
test) is listed in `tests/snippet_contexts.json`; a new snippet needs an entry there.

```sh
python3 tests/check_snippets.py     # needs flutter in PATH
```

## License

[MIT](LICENSE)
