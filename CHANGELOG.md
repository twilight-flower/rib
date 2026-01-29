# Changelog

## 0.1.2

- Focus book section on load of navigation page
- Fix TOC-parsing problems which led some books to be falsely rejected as invalid on the basis of TOCs which were actually valid.
- Fix problem where book navigation could end up duplicated if book made internal use of iframes between spine-sections.
- Fail faster when encountering an error in the course of opening an EPUB.
- Rename CLI `stylesheets` option to `stylesheet`.
- Clarify help-text on assorted CLI options.

## 0.1.1

- Pass fragment identifiers from navigation wrapper to book section

## 0.1.0

Initial release.
