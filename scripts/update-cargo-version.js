const fs = require('fs');
const path = require('path');

const version = process.argv[2];
const cargoTomlPath = path.join(__dirname, '../Cargo.toml');

fs.readFile(cargoTomlPath, 'utf8', (err, data) => {
    if (err) {
        console.error('Error reading Cargo.toml:', err);
        process.exit(1);
    }

    const updatedData = data.replace(
        /version = "(\d+\.\d+\.\d+)"/,
        `version = "${version}"`
    );

    fs.writeFile(cargoTomlPath, updatedData, 'utf8', (err) => {
        if (err) {
            console.error('Error writing Cargo.toml:', err);
            process.exit(1);
        }
        console.log(`Cargo.toml updated to version ${version}`);
    });
}); 