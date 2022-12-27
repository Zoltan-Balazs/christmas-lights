cross-release:
    mv ~/.cargo/config.toml ~/.cargo/config.toml.old
    cross build --release
    mv ~/.cargo/config.toml.old ~/.cargo/config.toml

cross:
    mv ~/.cargo/config.toml ~/.cargo/config.toml.old
    cross build
    mv ~/.cargo/config.toml.old ~/.cargo/config.toml

cross-release-target TARGET:
    mv ~/.cargo/config.toml ~/.cargo/config.toml.old
    cross build --release --target {{TARGET}}
    mv ~/.cargo/config.toml.old ~/.cargo/config.toml

cross-target TARGET:
    mv ~/.cargo/config.toml ~/.cargo/config.toml.old
    cross build --target {{TARGET}}
    mv ~/.cargo/config.toml.old ~/.cargo/config.toml