<p align="center">
  <img src="logo.png" alt="intern-mint" width="350">
</p>

## About
intern-mint is an implementation of byte slice interning.

slices are kept in a global hash-tables, sharded by the slices' hashes (to avoid locking the entire table).
