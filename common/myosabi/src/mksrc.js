// Make enum table script
'use strict';

const fs = require("fs");

const INPUT_PATH = __dirname + "/svc.txt"
const OUTPUT_PATH = __dirname + "/svc.rs"

const numbers = [];
const func_snakes = [];
const func_camels = [];
const comments = [];

const snakeToCamel = (p) => {
    return p
        .replace(/^\w/, (s) => s.toUpperCase())
        .replace(/_./g, (s) => s.charAt(1).toUpperCase());
};

const toHex = (value, length = 2) => {
    return value.toString(16).toUpperCase().padStart(length, '0')
}

let cursor = 0;
const x = fs.readFileSync(INPUT_PATH).toString().split(/\n/)
for (const ln in x) {
    const index = parseInt(ln)
    const line = x[index]
    if (line === '%end') break;
    if (line.startsWith('#')) continue;
    if (line.length < 1) continue;

    const components = line.split(/\|/);
    console.log('src', components);
    if (typeof components[0] !== 'string' || components[0].length === 0) continue;
    const func_name = components[0];
    const funcName = snakeToCamel(func_name);
    const num = components[1] | 0;
    const comment = components[2];
    if (num) {
        cursor = num;
    }

    numbers.push(cursor);
    func_snakes[cursor] = func_name;
    func_camels[cursor] = funcName;
    comments[cursor] = comment;

    console.log('intr', cursor, func_name, funcName);

    cursor++;
}

const class_name = "Function";
let lines = [
    '// SVC Function Numbers (AUTO GENERATED)',
    'use core::convert::TryFrom;',
    '',
];
lines.push('#[repr(u32)]');
lines.push('#[derive(Debug, Copy, Clone)]');
lines.push(`pub enum ${class_name} {`);
for (const i in numbers) {
    const index = numbers[i];
    const funcName = func_camels[index];
    // const func_name = func_snakes[index];
    const comment = comments[index];
    const remarks = [`[${index}]`, comment].filter(v => v && (v.length > 0));
    lines.push(`    /// ${remarks.join(' ')}`);
    lines.push(`    ${funcName} = ${index},`);
}
lines.push('}');
lines.push('');
lines.push(`impl TryFrom<u32> for ${class_name} {`);
lines.push(`    type Error = ();`);
lines.push('');
lines.push('    fn try_from(value: u32) -> Result<Self, Self::Error> {');
lines.push('        match value {');
for (const i in numbers) {
    const index = numbers[i];
    const funcName = func_camels[index];
    lines.push(`            ${index} => Ok(Self::${funcName}),`);
}
lines.push('            _ => Err(()),');
lines.push('        }');
lines.push('    }');
lines.push('}');
lines.push('');

// console.log(lines.join('\n'))
fs.writeFileSync(OUTPUT_PATH, lines.join('\n'))
