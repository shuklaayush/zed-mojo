# zed-mojo

Mojo support for Zed

Uses [tree-sitter-mojo](https://github.com/lsh/tree-sitter-mojo/) for parsing.

## Sample Settings

### LSP

```json:.zed/settings.json
{
  "lsp": {
    "mojo": {
      "settings": {
        "args": ["-I", "path/to/mojo/lib"]
      }
    }
  }
}
```

### Formatter

```json:.zed/settings.json
{
  "languages": {
    "Mojo": {
      "formatter": {
        "external": {
          "command": "pixi",
          "arguments": ["run", "mojo", "format", "-"]
        }
      }
    }
  }
}
```

### Debugger

```json:.zed/debug.json
[
  {
    "adapter": "mojo-lldb",
    "label": "my_label",
    "program": "${ZED_WORKTREE_ROOT}/path/to/your/program",
    "args": [
      "arg0",
      "arg1",
    ],
    "request": "launch",
    "runInTerminal": true
  }
]
```

## Acknowledgments
- [zed-mojo](https://github.com/bajrangCoder/zed-mojo)
