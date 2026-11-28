# Bugs

Found by hand-testing; each one names the cause where it is known and the
fix to make. Delete a bug when it is fixed (the CHANGELOG of the extension
records the fix).

## HTTP Client (`http-client-extension`)

- **An HTTP service group named by a constant is not found.**
  `@ImportHttpServices(group: EdgeFunctions.group, types: [Functions])`
  (Spring for Dart's Supabase starter; `static const String group =
  'supabase-functions'` in a dependency) matches no `registrations`
  pattern, which captures only a string literal, so the generated request
  goes to `{{baseUrl}}` instead of
  `spring.http.serviceclient.supabase-functions.base-url`. Fix: a
  `{ref}`-style group capture resolved through the `urls.assignments`
  lookup (`group = '...'`) in the project, and through a small table of
  the starters' known constants for those in dependencies.
- **Interface-level defaults of an `@HttpExchange` interface are not
  applied.** `@HttpExchange.of(url: '/x', method: HttpMethod.post,
  headers: ['X-Api=1'])` on the interface gives its prefix only; a method
  without its own method is generated as `GET`, and the headers are
  missing. Fix: a `defaults` pattern list read at class level, like
  `prefix`, applied to the routes of the class that set none.

## Flutter Tools (`flutter-extension`)
