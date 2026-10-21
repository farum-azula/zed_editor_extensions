; The lines of a code block whose fence names Dart, or no language (dartdoc's default), are
; highlighted as Dart (languages/dartdoc-code). A match per line, and the block as their host: Zed
; parses the lines of one block together, as one Dart layer, and each block apart from the others.
; Other languages are not injected: their outline and runnables would join the file's.
((code_block
  !language
  (code) @injection.content) @injection.host
  (#set! injection.language "Dartdoc Code"))

((code_block
  language: (info_string) @_language
  (code) @injection.content) @injection.host
  (#any-of? @_language "dart" "Dart")
  (#set! injection.language "Dartdoc Code"))
