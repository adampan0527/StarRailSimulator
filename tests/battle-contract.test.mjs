import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { URL } from 'node:url';
import Ajv2020 from 'ajv/dist/2020.js';

const load = (name) =>
  JSON.parse(readFileSync(new URL(`../contracts/${name}`, import.meta.url)));
const validate = new Ajv2020({ strict: true }).compile(
  load('battle-session.v1.schema.json'),
);

test('the fixture fits both consumers and uses distinct battle targets', () => {
  const session = load('battle-session.v1.example.json');
  assert.ok(validate(session), JSON.stringify(validate.errors));
  const units = [...session.input.team, ...session.input.waves.flat()];
  const ids = new Set(units.map((unit) => unit.unit_id));
  assert.equal(ids.size, units.length);
  for (const command of session.commands) {
    assert.ok(
      session.input.team.some((member) => member.unit_id === command.actor_id),
    );
    assert.ok(command.target_id === null || ids.has(command.target_id));
  }
});

test('rejects a floating profile reference and a seed beyond the portable range', () => {
  const session = load('battle-session.v1.example.json');
  const reference = load('battle-session.v1.example.json');
  reference.input.team[0].config = {
    profile_id: reference.input.team[0].character_form_id,
  };
  assert.equal(validate(reference), false);

  session.input.seed = 4_294_967_296;
  assert.equal(validate(session), false);
});

test('the replay schema accepts both Ultimate action types', () => {
  const session = load('battle-session.v1.example.json');
  session.commands[0].action = 'ultimate';
  assert.ok(validate(session), JSON.stringify(validate.errors));
  session.commands[0].action = 'enhanced_ultimate';
  assert.ok(validate(session), JSON.stringify(validate.errors));
});
