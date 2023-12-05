This is a copy of the glibc package directory as found in Bottlerocket.
We need this because this test assumes no outside dependencies
(i.e. no external kits and no alpha SDK).
However, we cannot build any Go or Rust packages without glibc as a build dependency.
