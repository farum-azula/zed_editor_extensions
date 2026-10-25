; Flutter main
((import_or_export
  (library_import
    (import_specification
      ("import"
        (configurable_uri
          (uri
            (string_literal) @_import
            (#match? @_import "package:flutter/")))))))
  ((function_signature
    name: (_) @run)
    (#eq? @run "main"))
  (#set! tag flutter-main))

; Flutter main that gets Flutter through another package (say a starter that exports it): a
; top-level main() whose body calls runApp
(((function_signature
    name: (_) @run) @_signature
  .
  (function_body) @_body)
  (#eq? @run "main")
  (#match? @_body "runApp\\s*\\(")
  (#set! tag flutter-main))

; Dart main: any top-level main() (a Flutter main gets its run tasks as well)
((function_signature
  name: (_) @run)
  (#eq? @run "main")
  (#set! tag dart-main))
