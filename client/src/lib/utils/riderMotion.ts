import * as THREE from 'three'
import { findBoneByName } from './characterAnimationUtils'
import { angleDelta } from './horseMovement'

type Limb = {
  base: THREE.Bone
  joint: THREE.Bone
  end: THREE.Bone
  target: THREE.Vector3
  pole: THREE.Vector3
  rotation: THREE.Quaternion
}

export class RiderMotion {
  private readonly torso: Limb | undefined
  private readonly arms: Limb[] = []
  private readonly legs: Limb[] = []
  private readonly saved: {
    bone: THREE.Bone
    position: THREE.Vector3
    rotation: THREE.Quaternion
  }[]
  private applied = false
  private readonly start = new THREE.Vector3()
  private readonly middle = new THREE.Vector3()
  private readonly direction = new THREE.Vector3()
  private readonly forward = new THREE.Vector3()
  private readonly bend = new THREE.Vector3()
  private readonly jointTarget = new THREE.Vector3()
  private readonly from = new THREE.Vector3()
  private readonly to = new THREE.Vector3()
  private readonly rotation = new THREE.Quaternion()
  private readonly parentRotation = new THREE.Quaternion()
  private readonly up = new THREE.Vector3(0, 1, 0)

  constructor(private readonly root: THREE.Object3D) {
    const limb = (names: string[]): Limb | undefined => {
      const [base, joint, end] = names.map((name) => findBoneByName(root, name))
      return base && joint && end
        ? {
            base,
            joint,
            end,
            target: new THREE.Vector3(),
            pole: new THREE.Vector3(),
            rotation: new THREE.Quaternion(),
          }
        : undefined
    }
    this.torso = limb(['Hips', 'Spine', 'Head'])
    for (const side of ['Left', 'Right']) {
      const arm = limb([`${side}Arm`, `${side}ForeArm`, `${side}Hand`])
      if (arm) this.arms.push(arm)
      const leg = limb([`${side}UpLeg`, `${side}Leg`, `${side}Foot`])
      if (leg) this.legs.push(leg)
    }
    const bones = [this.torso, ...this.arms, ...this.legs].flatMap((limb) =>
      limb ? [limb.base, limb.joint, limb.end] : []
    )
    this.saved = [...new Set(bones)].map((bone) => ({
      bone,
      position: new THREE.Vector3(),
      rotation: new THREE.Quaternion(),
    }))
  }

  restore() {
    if (!this.applied) return
    for (const saved of this.saved) {
      saved.bone.position.copy(saved.position)
      saved.bone.quaternion.copy(saved.rotation)
    }
    this.applied = false
  }

  apply(hipLift: number, handLift = 0, idleWeight = 0, facingYaw?: number) {
    this.restore()
    const { torso } = this
    const hips = torso?.base
    if (
      !hips?.parent ||
      !torso ||
      (hipLift <= 0 &&
        handLift === 0 &&
        idleWeight === 0 &&
        facingYaw === undefined)
    )
      return
    for (const saved of this.saved) {
      saved.position.copy(saved.bone.position)
      saved.rotation.copy(saved.bone.quaternion)
    }
    this.applied = true
    this.root.updateWorldMatrix(true, true)
    this.forward.set(0, 0, 1).transformDirection(this.root.matrixWorld)
    this.forward.y = 0
    this.forward.normalize()
    this.capture(torso)
    for (const arm of this.arms) {
      this.capture(arm)
      arm.target.y += handLift
      arm.target.addScaledVector(this.forward, -0.18 * idleWeight)
      arm.pole.lerp(this.direction.set(0, -arm.pole.length(), 0), idleWeight)
    }
    for (const leg of this.legs) this.capture(leg)
    torso.joint.getWorldPosition(this.start)
    this.direction.subVectors(torso.target, this.start)
    // Follow a constant torso-length arc around the anchored head.
    const ahead = this.direction.dot(this.forward)
    const backward =
      (Math.sqrt(
        Math.max(0, ahead ** 2 + 2 * this.direction.y * hipLift - hipLift ** 2)
      ) -
        Math.abs(ahead)) *
      0.5
    const rise =
      this.direction.y -
      Math.sqrt(
        Math.max(
          0,
          this.direction.y ** 2 - 2 * ahead * backward - backward ** 2
        )
      )
    hips.getWorldPosition(this.from).addScaledVector(this.forward, -backward)
    this.from.y += rise
    hips.position.copy(hips.parent.worldToLocal(this.from))
    hips.updateWorldMatrix(true, true)
    this.aim(torso.joint, torso.end, torso.target)
    this.orient(torso.end, torso.rotation)
    for (const leg of this.legs) this.solve(leg)
    for (const arm of this.arms) this.solve(arm)
    if (facingYaw !== undefined) {
      let turnYaw = facingYaw
      if (this.arms.length === 2) {
        this.arms[0].base.getWorldPosition(this.from)
        this.arms[1].base.getWorldPosition(this.to)
        this.direction.subVectors(this.from, this.to)
        turnYaw = angleDelta(
          Math.atan2(-this.direction.z, this.direction.x),
          Math.atan2(this.forward.x, this.forward.z) + facingYaw
        )
      }
      torso.joint.getWorldQuaternion(this.rotation)
      this.rotation.premultiply(
        this.parentRotation.setFromAxisAngle(this.up, turnYaw)
      )
      this.orient(torso.joint, this.rotation)
    }
  }

  private capture(limb: Limb) {
    limb.end.getWorldPosition(limb.target)
    limb.end.getWorldQuaternion(limb.rotation)
    limb.base.getWorldPosition(this.start)
    limb.joint.getWorldPosition(limb.pole).sub(this.start)
  }

  private solve(limb: Limb) {
    const { base, joint, end, target, pole } = limb
    base.getWorldPosition(this.start)
    joint.getWorldPosition(this.middle)
    end.getWorldPosition(this.to)
    const upperLength = this.start.distanceTo(this.middle)
    const lowerLength = this.middle.distanceTo(this.to)
    this.direction.subVectors(target, this.start)
    const distance = THREE.MathUtils.clamp(
      this.direction.length(),
      Math.abs(upperLength - lowerLength) + 1e-5,
      upperLength + lowerLength - 1e-5
    )
    this.direction.normalize()
    this.bend
      .copy(pole)
      .addScaledVector(this.direction, -pole.dot(this.direction))
      .normalize()
    const along =
      (upperLength ** 2 - lowerLength ** 2 + distance ** 2) / (2 * distance)
    const across = Math.sqrt(Math.max(0, upperLength ** 2 - along ** 2))
    this.jointTarget
      .copy(this.start)
      .addScaledVector(this.direction, along)
      .addScaledVector(this.bend, across)
    this.aim(base, joint, this.jointTarget)
    this.aim(joint, end, target)
    this.orient(end, limb.rotation)
  }

  private orient(bone: THREE.Bone, rotation: THREE.Quaternion) {
    bone.parent!.getWorldQuaternion(this.parentRotation).invert()
    bone.quaternion.copy(this.parentRotation.multiply(rotation))
    bone.updateWorldMatrix(true, true)
  }

  private aim(joint: THREE.Bone, end: THREE.Bone, target: THREE.Vector3) {
    joint.getWorldPosition(this.from)
    end.getWorldPosition(this.to).sub(this.from).normalize()
    this.from.subVectors(target, this.from).normalize()
    this.rotation.setFromUnitVectors(this.to, this.from)
    joint.getWorldQuaternion(this.parentRotation)
    this.rotation.multiply(this.parentRotation)
    joint.parent!.getWorldQuaternion(this.parentRotation).invert()
    joint.quaternion.copy(this.parentRotation.multiply(this.rotation))
    joint.updateWorldMatrix(true, true)
  }
}
