// Builds the Windows installer: a release build of the Strict service, put
// where Tauri bundles it from, then `tauri build` with the installer settings.
//
//   npm run installer
//
// The installer lands in target/release/bundle/nsis/. Building it changes
// nothing on this PC; running it installs Niwa and its service (admin).
//
// Updates: the installer is signed with the key in ~/.tauri/niwa-updater.key
// (or TAURI_SIGNING_PRIVATE_KEY), and latest.json is written beside it.
// Niwa's update address is the latest GitHub release of bento-v/niwa
// (Oct 22, 2026): publish both files as release v{version}.

import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const app = join(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(app, '..');
const run = (command, args, cwd) => execFileSync(command, args, { cwd, stdio: 'inherit' });

const key = join(homedir(), '.tauri', 'niwa-updater.key');
if (!process.env.TAURI_SIGNING_PRIVATE_KEY) {
  if (!existsSync(key)) {
    console.error(`No update signing key at ${key}. Updates can't be signed without it.`);
    process.exit(1);
  }
  process.env.TAURI_SIGNING_PRIVATE_KEY = key;
  process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ??= '';
}

// Tauri looks for sidecar programs under their target triple.
const triple = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (\S+)/m)[1];

run('cargo', ['build', '--release', '-p', 'niwa-service'], root);
const binaries = join(app, 'src-tauri', 'binaries');
mkdirSync(binaries, { recursive: true });
copyFileSync(join(root, 'target', 'release', 'niwa-service.exe'), join(binaries, `niwa-service-${triple}.exe`));

run(process.execPath, [join(app, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), 'build', '--config', 'src-tauri/tauri.installer.conf.json'], app);

// The file Niwa reads to learn about this version.
const config = JSON.parse(readFileSync(join(app, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const feed = config.plugins?.updater?.endpoints?.[0];
const bundle = join(root, 'target', 'release', 'bundle', 'nsis');
const installer = `${config.productName}_${config.version}_x64-setup.exe`;
// On GitHub the address is the latest release's latest.json. The installer
// it names is in this version's own release, not "latest", so a signature
// always goes with the installer it was made for.
const github = feed?.match(/^https:\/\/github\.com\/([^/]+\/[^/]+)\/releases\/latest\/download\//);
const tag = `v${config.version}`;
const installerUrl = github
  ? `https://github.com/${github[1]}/releases/download/${tag}/${installer}`
  : feed && new URL(installer, feed).href;
if (feed) {
  const notes = process.env.NIWA_RELEASE_NOTES ?? '';
  writeFileSync(
    join(bundle, 'latest.json'),
    JSON.stringify(
      {
        version: config.version,
        notes,
        pub_date: new Date().toISOString(),
        platforms: {
          'windows-x86_64': {
            signature: readFileSync(join(bundle, `${installer}.sig`), 'utf8'),
            url: installerUrl,
          },
        },
      },
      null,
      2,
    ) + '\n',
  );
  console.log(
    github
      ? `\nPublish ${installer} and latest.json as release ${tag} of github.com/${github[1]}.`
      : `\nFor the website: ${installer} and latest.json, both into ${new URL('.', feed).href}`,
  );
} else {
  console.log('\nNo update address in tauri.conf.json yet, so no latest.json.');
}
