((comment) @injection.content
  (#set! injection.language "comment"))

; All of the file's `///` comments, as one document of the Dartdoc language (languages/dartdoc),
; which highlights the code blocks in them. Combined, Zed hands them over a line each.
((documentation_comment) @injection.content
  (#match? @injection.content "^///")
  (#set! injection.language "dartdoc")
  (#set! injection.combined))
