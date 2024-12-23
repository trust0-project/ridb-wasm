module.exports = {
    repositoryUrl: 'https://github.com/trust0-project/ridb-wasm.git',
    branches: [
        { name: 'main' },
        { name: 'develop', prerelease: 'rc', channel: 'rc' },
        'v+([0-9])?(.{+([0-9]),x}).x',
    ],
    plugins: [
        '@semantic-release/commit-analyzer',
        '@semantic-release/release-notes-generator',
        '@semantic-release/npm',
        '@semantic-release/github',
        '@semantic-release/changelog',
        [
            '@semantic-release/git',
            {
                assets: ['CHANGELOG.md', 'package.json', 'package-lock.json'],
                message:
                    'chore(release): ${nextRelease.version} [skip ci]\n\n${nextRelease.notes}',
            },
        ],
    ],
};
