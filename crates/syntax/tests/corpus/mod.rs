//! Real source, in every language the scope table makes a claim about.
//!
//! Not fixtures for one assertion — a shared corpus. Every selector in
//! `theme::code::SCOPES` has to claim something *here*, so a language is
//! present because some scope needs it: C for the preprocessor and `goto`,
//! YAML for anchors, `.patch` for `markup.inserted`. Adding a scope usually
//! means adding the construct that produces it.
//!
//! Idiomatic rather than minimal, because a grammar's scopes depend on
//! context: a `class` keyword is scoped differently when it is followed by a
//! name the grammar recognises.

/// `(path, source)`. The path is all the detector gets, so it has to be the
/// name a real file would have.
pub const FILES: &[(&str, &str)] = &[
    (
        "src/main.rs",
        r##"//! A module comment.
use std::collections::HashMap;

/// What a thing is.
#[derive(Debug, Clone)]
pub struct Thing {
    pub name: String,
    count: u32,
}

pub trait Greet {
    fn greet(&self) -> String;
}

pub enum Colour {
    Red,
    Green,
}

pub union Bits {
    whole: u32,
    half: [u16; 2],
}

impl Thing {
    pub fn new(name: &str, count: u32) -> Self {
        let tab = '\t';
        let text = "a \"quoted\" string\n";
        println!("{name} has {count} {tab}{text}");
        Self {
            name: name.to_owned(),
            count,
        }
    }

    fn total(&self) -> u32 {
        if self.count > 0 { self.count * 2 } else { 0 }
    }
}

mod inner {
    pub const LIMIT: usize = 20_000;
}
"##,
    ),
    (
        "src/thing.c",
        r#"/* A block comment. */
#include <stdio.h>
#define LIMIT 20000

typedef struct {
    char name[64];
    unsigned int count;
} thing_t;

union bits {
    unsigned int whole;
    unsigned short half[2];
};

enum colour { RED, GREEN };

int main(int argc, char **argv) {
    char tab = '\t';
    const char *text = "a \"quoted\" string\n";
    thing_t t = {0};
    if (argc < 2)
        goto done;
    printf("%s has %d%c\n", t.name, t.count, tab);
done:
    return 0;
}
"#,
    ),
    (
        "src/thing.ts",
        r#"// A line comment.
import { readFile } from "node:fs/promises";

export interface Greet {
    greet(): string;
}

type Name = string | null;

@sealed
export class Thing implements Greet {
    static LIMIT = 20000;
    private readonly pattern = /^[a-z]+\d*$/gi;

    constructor(
        public name: string,
        private count: number = 0,
    ) {}

    greet(): string {
        const tab = "\t";
        return `hello ${this.name}, you have ${this.count + 1} of them${tab}`;
    }
}

export const make = (name: string): Thing => new Thing(name);
console.log(make("x").greet());
"#,
    ),
    (
        "src/thing.py",
        r#"# A line comment.
import os
from dataclasses import dataclass


@dataclass
class Thing(Base):
    """What a thing is."""

    name: str
    count: int = 0

    def greet(self, times: int = 1) -> str:
        pattern = r"^[a-z]+\d*$"
        text = "a \"quoted\" string\n"
        return f"hello {self.name}, {times} times {text} {pattern}"


def main() -> None:
    thing = Thing(name="x")
    print("%s -> %s" % (thing.name, thing.greet()))
    os.environ["HOME"]
"#,
    ),
    (
        "src/thing.go",
        r#"// Package thing does something.
package thing

import (
	"fmt"
	"strings"
)

type Greeter interface {
	Greet() string
}

type Thing struct {
	Name  string
	Count int
}

const Limit = 20000

func (t *Thing) Greet() string {
	tab := '\t'
	raw := `a raw string`
	if t.Count > 0 {
		return fmt.Sprintf("%s %d%c%s", t.Name, t.Count, tab, raw)
	}
	return strings.ToUpper(t.Name)
}
"#,
    ),
    (
        "src/Thing.java",
        r#"// A line comment.
package com.example.thing;

import java.util.List;

/** What a thing is. */
@Deprecated
public class Thing extends Base implements Greet {
    private static final int LIMIT = 20000;
    private final String name;

    public Thing(String name) {
        this.name = name;
    }

    @Override
    public String greet() {
        char tab = '\t';
        return String.format("%s%c", this.name, tab);
    }
}
"#,
    ),
    (
        "src/thing.rb",
        r#"# A line comment.
require "json"

module Greeting
  LIMIT = 20_000

  class Thing < Base
    attr_reader :name

    def initialize(name, count: 0)
      @name = name
      @count = count
    end

    def greet
      pattern = /^[a-z]+\d*$/
      "hello #{@name}, #{@count} times #{pattern}"
    end
  end
end
"#,
    ),
    (
        "src/index.html",
        r#"<!DOCTYPE html>
<!-- A comment. -->
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Thing</title>
  </head>
  <body class="page" data-count="3">
    <h1 id="title">Hello &amp; welcome</h1>
    <a href="https://example.com">a link</a>
    <script>
      const x = 1;
    </script>
  </body>
</html>
"#,
    ),
    (
        "src/thing.css",
        r#"/* A comment. */
@import url("other.css");

:root {
  --accent: #cba6f7;
}

.page > a:hover,
#title {
  color: var(--accent);
  font-size: 1.5rem;
  margin: 0 auto;
  content: "a string";
}

@media (min-width: 40rem) {
  .page {
    display: grid;
  }
}
"#,
    ),
    (
        "package.json",
        r#"{
  "name": "codediff",
  "version": "0.10.1",
  "private": true,
  "count": 20000,
  "scripts": {
    "build": "cargo build"
  },
  "keywords": ["diff", "review"]
}
"#,
    ),
    (
        "compose.yaml",
        r#"# A comment.
version: "3"
defaults: &defaults
  restart: always
  count: 3
services:
  app:
    <<: *defaults
    image: example/app:1.0
    command: ["cargo", "run"]
    environment:
      - RUST_LOG=debug
"#,
    ),
    (
        "Cargo.toml",
        r#"# A comment.
[package]
name = "codediff"
version = "0.10.1"
edition = "2024"

[dependencies]
ratatui = { version = "0.29", features = ["crossterm"] }
enabled = true
count = 20000
"#,
    ),
    (
        "README.md",
        r#"# A heading

Some **bold** text, some *italic* text, some `inline code`, and a
[link label](https://example.com) with a trailing sentence.

> A block quote, which the grammar scopes on its own.

- a list item
- another one

1. a numbered item

```rust
fn main() {}
```

---

| a | b |
|---|---|
| 1 | 2 |
"#,
    ),
    (
        "scripts/build.sh",
        r#"#!/usr/bin/env bash
# A comment.
set -euo pipefail

LIMIT=20000
name="${1:-thing}"

greet() {
    local text="hello ${name}"
    printf '%s\n' "$text"
}

if [[ -n "$name" ]]; then
    greet
fi

for f in *.rs; do
    echo "$f"
done
"#,
    ),
    (
        "src/query.sql",
        r#"-- A comment.
SELECT t.name, COUNT(*) AS total
FROM things AS t
WHERE t.name LIKE 'a%'
  AND t.count > 20000
GROUP BY t.name
ORDER BY total DESC
LIMIT 10;
"#,
    ),
    (
        "src/thing.lua",
        r#"-- A comment.
local json = require("json")

local Thing = {}
Thing.__index = Thing

function Thing.new(name, count)
    local self = setmetatable({}, Thing)
    self.name = name
    self.count = count or 0
    return self
end

function Thing:greet()
    if self.count > 0 then
        return string.format("%s %d", self.name, self.count)
    end
    return "hello " .. self.name
end

return Thing
"#,
    ),
    (
        "change.patch",
        r#"diff --git a/src/main.rs b/src/main.rs
index 1234567..89abcde 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,4 @@
 fn main() {
-    println!("old");
+    println!("new");
 }
"#,
    ),
    (
        // Broken on purpose. A reviewer sees more of this than anyone —
        // half-finished agent output is the reason this program exists — and
        // a grammar that says "this is wrong" is worth showing.
        "src/legacy.py",
        r#"# Python 2 leftovers a grammar refuses.
def compare(a, b):
    return a <> b
"#,
    ),
    (
        "src/broken.json",
        r#"{
  "name": "codediff",,
  "count": 3
}
"#,
    ),
];
