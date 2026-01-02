# WorldBoxNMLPack

A simple CLI tool for packing [WorldBox](https://www.superworldbox.com/) mods that use the [NeoModLoader](https://github.com/WorldBoxOpenMods/ModLoader).

## Features

- **Automated Packing**: Quickly zip your mod files into a structure compatible with NML.
- **Build Integration**: Option to trigger your build command (e.g., `dotnet build`) before packing.
- **Flexible Configuration**: Customize included source folders, assets, and additional files via command-line options.

## Usage

Execute the command below from the project root:

```shell
nmlpack -c
```

More options:

```shell
> nmlpack -h
Usage: nmlpack [OPTIONS]

Options:
      --assets <ASSETS>    Asset directories to be included in the package [default: assets]
      --build <BUILD>      The command used to build the project [default: "dotnet build -p:DebugType=Portable"]
  -c, --compile            Whether to build binary
      --include <INCLUDE>  Additional files or directories to include [default: Locals LICENSE default_config.json icon.png mod.json]
  -o, --output <OUTPUT>    The final output path of the packed zip file
      --pdb                Whether to include PDB files
      --sources <SOURCES>  Source code directories [default: Code code src]
  -h, --help               Print help
  -V, --version            Print version
```

## License

This project is licensed under the [MIT License](/LICENSE) © 2025 557.
