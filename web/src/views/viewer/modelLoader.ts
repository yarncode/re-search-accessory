import * as THREE from 'three'
import { VRMLLoader } from 'three/examples/jsm/loaders/VRMLLoader.js'
import occtimportjs from 'occt-import-js'
import wasmUrl from 'occt-import-js/dist/occt-import-js.wasm?url'

export type ModelTransform = {
  offset: [number, number, number]
  scale: [number, number, number]
  rotate: [number, number, number]
}

let occtPromise: Promise<Awaited<ReturnType<typeof occtimportjs>>> | null = null

function getOcct() {
  occtPromise ??= occtimportjs({ locateFile: () => wasmUrl })

  return occtPromise
}

export async function loadStep(bytes: Uint8Array): Promise<THREE.Group> {
  const occt = await getOcct()
  const result = occt.ReadStepFile(bytes, null)
  const group = new THREE.Group()

  for (const mesh of result?.meshes ?? []) {
    const position = mesh.attributes?.position?.array

    if (!position)
      continue

    const geometry = new THREE.BufferGeometry()

    geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(position), 3))

    const normal = mesh.attributes?.normal?.array

    if (normal)
      geometry.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(normal), 3))

    if (mesh.index?.array)
      geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(mesh.index.array), 1))

    if (!normal)
      geometry.computeVertexNormals()

    const color = mesh.color
      ? new THREE.Color(mesh.color[0], mesh.color[1], mesh.color[2])
      : new THREE.Color('#9db8ff')

    group.add(new THREE.Mesh(
      geometry,
      new THREE.MeshStandardMaterial({ color, metalness: 0.15, roughness: 0.55 }),
    ))
  }

  return group
}

export function loadWrl(bytes: Uint8Array): THREE.Object3D {
  const text = new TextDecoder().decode(bytes)
  const loader = new VRMLLoader()

  return loader.parse(text, '') as THREE.Object3D
}

/** KiCad applies the model offset first, then the rotation (degrees), then the
 *  scale, so the hierarchy must be rotate -> translate -> scale -> model. */
export function applyKiCadTransform(object: THREE.Object3D, transform: ModelTransform): THREE.Object3D {
  const scaled = new THREE.Group()

  scaled.position.set(...transform.offset)
  scaled.scale.set(...transform.scale)
  scaled.add(object)

  const rotated = new THREE.Group()

  rotated.rotation.set(
    THREE.MathUtils.degToRad(transform.rotate[0]),
    THREE.MathUtils.degToRad(transform.rotate[1]),
    THREE.MathUtils.degToRad(transform.rotate[2]),
    'XYZ',
  )
  rotated.add(scaled)

  return rotated
}

export function disposeObject(object: THREE.Object3D) {
  object.traverse(child => {
    if (child instanceof THREE.Mesh) {
      child.geometry.dispose()

      const material = child.material

      if (Array.isArray(material))
        material.forEach(entry => entry.dispose())
      else
        material.dispose()
    }
  })
}

export function base64ToBytes(value: string): Uint8Array {
  return Uint8Array.from(atob(value), char => char.charCodeAt(0))
}
