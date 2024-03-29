# Project Setup

To start a new songbook, navigate to an empty directory with your command line and run:

```bash
bard init
```

This will initialize a new Bard project.

A Bard project is configured with a file named `bard.toml` in the root of the folder,
written in the [TOML](https://toml.io/en/) format.

Apart from the `bard.toml` file, the project folder contains these subfolders:
 - `songs`: Contains Markdown songs files.
 - `output`: Outputs will be placed here.

Let's go through the `bard.toml` file.

### Inputs

The top of the file specifies the Bard version number and inputs:

```toml
version = 2

songs = [
    "yippie.md",
]

notation = "english"
```

The `songs` field is an array of filenames in the `songs` directory from which Bard loads the songs.
It may also contain globs (filenames with wildcards)
or be just one glob &ndash; to load all the `.md` files from `songs` in bulk, set it as:

```toml
songs = "*.md"
```

The songs are added in the songbook in the same order as they are defined
in the `songs` field. By reordering the song files in the `songs` field,
you control their order in the final output. Files matched by globs are ordered
alphabetically.

The `notation` field defines the language-specific variant of chords
used in the songs. This is only important if you use transposition,
see the [Transposition and Notation](./transposition.md) chapter for details.

### Outputs

The outputs array defines what output files should be generated:

```toml
[[output]]
file = "songbook.pdf"

[[output]]
file = "songbook.html"
```

The default configuration lists two outputs: a PDF file and an HTML file.

##### ToC order

By default, the table of contents in both HTML and PDF outputs follows the same order
in which the songs are specified in inputs.

To have the ToC sorted alphabetically, use the `toc_sort` setting. For example:

```toml
[[output]]
file = "songbook.pdf"
toc_sort = true
```

### Book metadata

The final section describes the book:

```toml
[book]
title = "My Songbook"
subtitle = "(You can edit that title!)"
chorus_label = "Ch"
title_note = "(And this note too...)"
```

Here you can configure the book's main title, its subtitle (optional),
the label to be used for choruses, and a 'title note', which is a small piece of text
on the bottom of the title page (optional).

See the [bard.toml reference](./bard.toml.md) for the complete list of options.

### Building the book and next steps

To build the project, use the following command:

```bash
bard make
```

If everything went well, you should see a PDF and an HTML file in the `output` directory.

Once you are happy with how the project is set up, you'll probably want to start [Writing Songs](./songs.md).
