# Migration to version 2

There were a few incompatible changes in bard v 2.x. To convert your project to bard 2.x, perform the following updates...

### `bard.toml` version

The `bard.toml` file now contains an explicit version field corresponding to the program major version.
Add the following line at the top of the file:

```toml
version = 2
```

### PDF format and post-processing

In bard 1.x, `.tex` files were generated and rendered as PDFs using the `process` field.
This is now done by bard automatically. The `"tex"` format is replaced by the `"pdf"` format and the `"process"` field is no longer recognized.

To update a PDF `[[output]]`:

- Specify a `file` with a `.pdf` extension.
- If you used an explicit `format` field, change it to `"pdf"`.
- Remove the `process` field. If you used the `process` field for a purpose other than TeX invocation, see [Scripts](./scripts.md).
- Optionally specify [ToC sorting](./tex.md#toc-sorting-configuration).

### Hovorka

The Hovorka format now needs to explicitly defined so as to be distinguished from the general XML AST output. Add the following in the relevant `[[output]]`:

```toml
format = "hovorka"
```

### Templates

If you are using the default template without changes, it is recommended to delete it as bard now by default uses built-in templates without saving them to disk.

Templates have been changed significantly between the two major versions. If you used customizations, please refer to [Templates - upgrading](./templates.md#upgrading). The default templates can also be easily obtained by specifying a non-existing file in the output's `tamplate` field - the file will be generated with default template content.