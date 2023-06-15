# Version 0.5.0

- Made `ReaderData`, `ReadError`, and `WriteError` public
- Use `From` trait instead of custom functions for conversions between
  `Reader` and `ReaderData`
- Added constants for status codes
- Fix serialisation

# Version 0.4.0

- Added destructuring methods for `Reader` and `Writer`
- Added `Reader` constructor from raw parts
- Changed `Reader` and `Writer` methods to return specific error types

# Version 0.3.4

- Fixed regression in `Writer`

# Version 0.3.3

- Improve performance by a lot
- Fix documentation
- Update `itertools` dependency

# Version 0.3.2

- Update dependencies
- Address compiler warnings (Rust 1.46.0)

# Version 0.3.1

- Added license file

# Version 0.3.0

- Update to Rust 2018 (1.35.0)

# Version 0.2.0

- Implemented a writer
- Support attributes in xml tags
- Renamed reader `event` method to `hepeup`
- Separate `xml_header` method for reading xml headers
- Added changelog
