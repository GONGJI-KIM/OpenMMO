import * as THREE from 'three'
import { clone } from 'three/examples/jsm/utils/SkeletonUtils.js'
import type { GLTF } from 'three/examples/jsm/loaders/GLTFLoader.js'

export const HORSE_MODEL_PATH = '/models/mounts/horse.glb'
export const RIDING_ANIMATION_PATH = '/models/animations/riding.glb'
const RUN_STRIDE_DURATION = 20 / 30

export class HorseMount {
  readonly root: THREE.Object3D
  readonly seat: THREE.Object3D
  riderHipLift = 0
  riderHandLift = 0
  riderIdleWeight = 0
  riderBaseOffsetY = 0
  private readonly runSeatHeight: number
  private readonly seatPosition = new THREE.Vector3()
  private readonly mixer: THREE.AnimationMixer
  private readonly actions = new Map<string, THREE.AnimationAction>()
  private current: THREE.AnimationAction | null = null

  constructor(gltf: GLTF) {
    this.root = clone(gltf.scene)
    this.seat = this.root.getObjectByName('RideSeat') ?? this.root
    this.mixer = new THREE.AnimationMixer(this.root)
    this.root.traverse((node) => {
      if (node instanceof THREE.Mesh) {
        node.castShadow = true
        node.receiveShadow = true
      }
    })
    for (const clip of gltf.animations) {
      this.actions.set(clip.name, this.mixer.clipAction(clip))
    }
    let height = 0
    const run = this.actions.get('run')
    if (run) {
      run.play()
      for (let i = 0; i < 8; i++) {
        this.mixer.setTime((i / 8) * RUN_STRIDE_DURATION)
        this.seat.getWorldPosition(this.seatPosition)
        height += this.root.worldToLocal(this.seatPosition).y / 8
      }
      run.stop()
      this.mixer.setTime(0)
    }
    this.runSeatHeight = height
  }

  update(dt: number, speed: number) {
    const name = speed < 0.1 ? 'idle' : speed < 3 ? 'walk' : 'run'
    const next = this.actions.get(name)
    if (next && next !== this.current) {
      next.reset().play()
      if (this.current) next.crossFadeFrom(this.current, 0.2, false)
      this.current = next
    }
    if (this.current) {
      this.current.timeScale =
        name === 'idle' ? 1 : speed / (name === 'walk' ? 2 : 6)
    }
    this.mixer.update(dt)
    const run = this.actions.get('run')
    const weight = run?.isRunning() ? run.getEffectiveWeight() : 0
    const runPhase = ((run?.time ?? 0) * Math.PI * 2) / RUN_STRIDE_DURATION
    this.seat.getWorldPosition(this.seatPosition)
    const seatY = this.root.worldToLocal(this.seatPosition).y
    this.riderBaseOffsetY = (this.runSeatHeight - 0.02 - seatY) * weight
    const bounce = 0.045 * (1 - Math.cos(runPhase)) * weight
    this.riderHipLift = Math.max(0, bounce, -this.riderBaseOffsetY)
    const idle = this.actions.get('idle')
    const idleWeight = idle?.isRunning() ? idle.getEffectiveWeight() : 0
    this.riderIdleWeight = idleWeight
    this.riderHandLift =
      -0.2 * idleWeight +
      Math.sin((this.mixer.time * Math.PI) / 2) * 0.004 +
      Math.sin(runPhase - 0.4) * 0.015 * weight
  }

  dispose() {
    this.mixer.stopAllAction()
    this.mixer.uncacheRoot(this.root)
  }
}
