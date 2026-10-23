; Flutter main
((import_or_export
  (library_import
    (import_specification
      ("import"
        (configurable_uri
          (uri
            (string_literal) @_import
            (#match? @_import "package:flutter/(material|widgets|cupertino).dart")
            (#not-match? @_import "package:flutter_test/flutter_test.dart")
            (#not-match? @_import "package:test/test.dart")))))))
  ((function_signature
    name: (_) @run)
    (#eq? @run "main"))
  (#set! tag flutter-main))

; Dart main: any top-level main() (a Flutter main gets its run tasks as well)
((function_signature
  name: (_) @run)
  (#eq? @run "main")
  (#set! tag dart-main))
