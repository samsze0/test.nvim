# test.nvim

A collection of tools and libs for testing neovim plugins.

![](assets/demo.png)

## How it works

This project comes with an executable `nvim-test-runner` that does the following:
- Setups any dependencies that your plugin replies on, cloning them to `.tests` and adding them to vim runtimepath
- Probe your plugin for test files and run them in neovim headless mode
- Report the test results in a pretty format

## Usage

Create a file named `nvim-test-runner.json` in your plugin root directory

```json
{
    "test_dependencies": [
        {
            "url": "https://github.com/samsze0/utils.nvim"
        }
    ]
}
```

```shell
nvim-test-runner
```

## License

MIT