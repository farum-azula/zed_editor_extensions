/**
 * The `///` documentation comments of a Dart file, as Zed hands them over: the Dart language
 * injects this grammar into all of a file's `///` comments at once (`injection.combined`), so it
 * sees them one after the other, each on its own line, without the code and indentation between.
 *
 * It finds the fenced code blocks of the Markdown that dartdoc reads, and gives each line of their
 * code a `code` node: the text after `///`, its newline included. Their injection
 * (languages/dartdoc/injections.scm) highlights them as Dart, the language dartdoc assumes when a
 * fence names none. The rest of a comment stays text.
 */

module.exports = grammar({
  name: 'dartdoc',

  // Every character belongs to a line: nothing is skipped between tokens.
  extras: () => [],

  rules: {
    document: $ => repeat(choice($._line, $.code_block)),

    _line: $ => prec.right(seq($._prefix, optional($._text), optional($._newline))),

    code_block: $ => prec.right(seq(
      $._prefix,
      $.fence,
      optional(field('language', $.info_string)),
      optional($._info_rest),
      optional($._newline),
      repeat(seq($._prefix, $.code)),
      optional(seq($._prefix, $.fence, optional($._newline))),
    )),

    _prefix: () => token('///'),

    // Up to three spaces, then three or more backticks or tildes: a fence opens or closes a code
    // block. Above the rest of a line in precedence, so that the line is not text, or code.
    fence: () => token(prec(1, /[ \t]{0,3}(```+|~~~+)[ \t]*/)),

    info_string: () => /[^\s`]+/,

    // What follows the language (`title="..."`); it starts with a space, so the language is not
    // taken for part of it.
    _info_rest: () => /[ \t][^\n]*/,

    // A line of a code block: everything after `///`, with the newline, so that the lines of a
    // block stay lines once injected.
    code: () => /[^\n]+\n?|\n/,

    _text: () => /[^\n]+/,

    _newline: () => '\n',
  },
});
