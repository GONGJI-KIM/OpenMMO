import { describe, expect, it } from 'vitest'
import * as THREE from 'three'
import { RiderMotion } from './riderMotion'

function rider() {
  const root = new THREE.Group()
  const bones = ['Hips', 'Spine', 'Spine1', 'Spine2', 'Neck', 'Head'].map(
    (name, i) => {
      const bone = new THREE.Bone()
      bone.name = name
      bone.position.y = i ? 0.12 : 0
      return bone
    }
  )
  bones.slice(1).forEach((bone, i) => bones[i].add(bone))
  const legs = ['Left', 'Right'].flatMap((side, i) => {
    const thigh = new THREE.Bone()
    const knee = new THREE.Bone()
    const foot = new THREE.Bone()
    thigh.name = `${side}UpLeg`
    knee.name = `${side}Leg`
    foot.name = `${side}Foot`
    thigh.position.set(i ? -0.1 : 0.1, -0.1, 0)
    knee.position.set(0, -0.35, 0.4)
    foot.position.set(0, -0.4, -0.3)
    bones[0].add(thigh)
    thigh.add(knee)
    knee.add(foot)
    return [thigh, knee, foot]
  })
  const arms = ['Left', 'Right'].flatMap((side, i) => {
    const arm = new THREE.Bone()
    const elbow = new THREE.Bone()
    const hand = new THREE.Bone()
    arm.name = `${side}Arm`
    elbow.name = `${side}ForeArm`
    hand.name = `${side}Hand`
    const sign = i ? -1 : 1
    arm.position.set(sign * 0.15, 0.08, 0)
    elbow.position.set(sign * 0.12, -0.25, -0.08)
    hand.position.set(0, 0, 0.3)
    bones[3].add(arm)
    arm.add(elbow)
    elbow.add(hand)
    return [arm, elbow, hand]
  })
  const mesh = new THREE.SkinnedMesh(
    new THREE.BufferGeometry(),
    new THREE.MeshBasicMaterial()
  )
  root.add(bones[0], mesh)
  root.updateMatrixWorld(true)
  mesh.bind(new THREE.Skeleton([...bones, ...arms, ...legs]))
  root.position.set(1200, 50, 2300)
  root.rotation.y = 1.2
  root.updateMatrixWorld(true)
  return { root, bones, arms, legs, motion: new RiderMotion(root) }
}

const world = (bone: THREE.Bone) => bone.getWorldPosition(new THREE.Vector3())

describe('rider motion', () => {
  it('tucks both elbows toward the body while idle without stretching the arms', () => {
    const { root, arms, motion } = rider()
    const before = arms.map((bone) => world(bone))
    motion.apply(0, -0.2, 1)
    for (const i of [0, 3]) {
      const shoulder = world(arms[i])
      const elbow = world(arms[i + 1])
      const hand = world(arms[i + 2])
      expect(elbow.y).toBeLessThan(shoulder.y)
      expect(Math.abs(root.worldToLocal(elbow.clone()).x)).toBeLessThan(
        Math.abs(root.worldToLocal(before[i + 1].clone()).x)
      )
      expect(shoulder.distanceTo(elbow)).toBeCloseTo(
        before[i].distanceTo(before[i + 1]),
        5
      )
      expect(elbow.distanceTo(hand)).toBeCloseTo(
        before[i + 1].distanceTo(before[i + 2]),
        5
      )
    }
  })
  it('lowers and pulls back the hands without moving the anchored head', () => {
    const { root, arms, bones, motion } = rider()
    const targets = [world(arms[2]), world(arms[5])]
    const head = world(bones[5])
    const forward = new THREE.Vector3(0, 0, 1).transformDirection(
      root.matrixWorld
    )
    for (const [lift, idleWeight] of [
      [-0.2, 1],
      [0, 1],
      [-0.1, 0.5],
      [-0.015, 0],
      [0.015, 0],
    ]) {
      motion.apply(0, lift, idleWeight)
      for (const i of [0, 1]) {
        const expected = targets[i]
          .clone()
          .addScaledVector(forward, -0.18 * idleWeight)
        expected.y += lift
        expect(world(arms[i * 3 + 2]).distanceTo(expected)).toBeLessThan(1e-5)
      }
      expect(world(bones[5]).distanceTo(head)).toBeLessThan(1e-5)
    }
  })
  it('starts smoothly from a slightly reclined pose while keeping the head anchored', () => {
    const { root, bones, motion } = rider()
    bones[2].position.z = -0.035
    root.updateMatrixWorld(true)
    const hips = world(bones[0])
    const head = world(bones[5])
    motion.apply(0.000001)
    expect(world(bones[0]).distanceTo(hips)).toBeLessThan(0.001)
    expect(world(bones[5]).distanceTo(head)).toBeLessThan(1e-5)
    motion.apply(0.09)
    expect(world(bones[5]).distanceTo(head)).toBeLessThan(1e-5)
  })
  it('moves the pelvis backward and hinges at the waist without bending the upper back', () => {
    const { root, bones, motion } = rider()
    const rotations = bones.map((bone) => bone.quaternion.clone())
    const hipBefore = root.worldToLocal(world(bones[0]))
    motion.apply(0.09)
    expect(root.worldToLocal(world(bones[0])).z).toBeLessThan(hipBefore.z - 0.1)
    expect(bones[0].quaternion.angleTo(rotations[0])).toBeLessThan(1e-5)
    expect(bones[1].quaternion.angleTo(rotations[1])).toBeGreaterThan(0.1)
    for (const i of [2, 3, 4]) {
      expect(bones[i].quaternion.angleTo(rotations[i])).toBeLessThan(1e-5)
    }
  })
  it('extends the knees while holding both feet in place', () => {
    const { legs, motion } = rider()
    const angle = (i: number) =>
      world(legs[i])
        .sub(world(legs[i + 1]))
        .angleTo(world(legs[i + 2]).sub(world(legs[i + 1])))
    const targets = [world(legs[2]), world(legs[5])]
    const angles = [angle(0), angle(3)]
    motion.apply(0.09)
    for (const i of [0, 1]) {
      expect(angle(i * 3)).toBeGreaterThan(angles[i])
      expect(world(legs[i * 3 + 2]).distanceTo(targets[i])).toBeLessThan(1e-5)
    }
  })
  it('holds both hands on the reins while the torso bends', () => {
    const { arms, motion } = rider()
    const hands = [arms[2], arms[5]]
    const targets = hands.map(world)
    motion.apply(0.09)
    hands.forEach((hand, i) =>
      expect(world(hand).distanceTo(targets[i])).toBeLessThan(1e-5)
    )
  })
  it('raises the hips while holding the head position and gaze, without shortening bones', () => {
    const { root, bones, motion } = rider()
    const hip = world(bones[0])
    const head = world(bones[5])
    const gaze = bones[5].getWorldQuaternion(new THREE.Quaternion())
    const lengths = bones
      .slice(1)
      .map((bone, i) => world(bone).distanceTo(world(bones[i])))
    motion.apply(0.09)
    root.updateMatrixWorld(true)
    expect(world(bones[0]).y - hip.y).toBeGreaterThan(0.005)
    expect(world(bones[0]).y - hip.y).toBeLessThan(0.04)
    expect(world(bones[5]).distanceTo(head)).toBeLessThan(1e-5)
    expect(
      bones[5].getWorldQuaternion(new THREE.Quaternion()).angleTo(gaze)
    ).toBeLessThan(1e-5)
    bones
      .slice(1)
      .forEach((bone, i) =>
        expect(world(bone).distanceTo(world(bones[i]))).toBeCloseTo(
          lengths[i],
          5
        )
      )
  })

  it('restores the animated pose when stopped and never accumulates offsets', () => {
    const { root, bones, motion } = rider()
    const positions = bones.map(world)
    motion.apply(0.09)
    const raised = world(bones[0])
    for (let i = 0; i < 120; i++) motion.apply(0.09)
    expect(world(bones[0]).distanceTo(raised)).toBeLessThan(1e-5)
    motion.apply(0)
    root.updateMatrixWorld(true)
    bones.forEach((bone, i) =>
      expect(world(bone).distanceTo(positions[i])).toBeLessThan(1e-5)
    )
  })
})
