/** Draws CPU-rendered `rgb0` video frames (4 bytes per pixel, 4th byte ignored) on a canvas. */
export interface FrameDrawer {
  draw(pixels: Uint8Array, width: number, height: number): void;
}

const VERTEX = `#version 300 es
in vec2 position;
out vec2 uv;
void main() {
  // Row 0 of the frame is the top of the picture.
  uv = vec2(position.x + 1.0, 1.0 - position.y) * 0.5;
  gl_Position = vec4(position, 0.0, 1.0);
}`;
const FRAGMENT = `#version 300 es
precision mediump float;
in vec2 uv;
uniform sampler2D frame;
out vec4 color;
void main() { color = vec4(texture(frame, uv).rgb, 1.0); }`;

/** WebGL2 texture upload: much cheaper than putImageData for 1080p60. Null if unavailable. */
export function createWebGlDrawer(canvas: HTMLCanvasElement): FrameDrawer | null {
  let gl: WebGL2RenderingContext | null = null;
  try {
    gl = canvas.getContext('webgl2', {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      desynchronized: true,
      powerPreference: 'high-performance',
    });
  } catch {
    return null;
  }
  if (!gl) return null;
  const compile = (type: number, source: string) => {
    const shader = gl!.createShader(type)!;
    gl!.shaderSource(shader, source);
    gl!.compileShader(shader);
    return shader;
  };
  const program = gl.createProgram()!;
  gl.attachShader(program, compile(gl.VERTEX_SHADER, VERTEX));
  gl.attachShader(program, compile(gl.FRAGMENT_SHADER, FRAGMENT));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) return null;
  gl.useProgram(program);

  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
  const position = gl.getAttribLocation(program, 'position');
  gl.enableVertexAttribArray(position);
  gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);

  gl.bindTexture(gl.TEXTURE_2D, gl.createTexture());
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

  let textureWidth = 0;
  let textureHeight = 0;
  return {
    draw(pixels, width, height) {
      const g = gl!;
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }
      g.viewport(0, 0, width, height);
      const data = pixels.subarray(0, width * height * 4);
      if (width !== textureWidth || height !== textureHeight) {
        g.texImage2D(g.TEXTURE_2D, 0, g.RGBA8, width, height, 0, g.RGBA, g.UNSIGNED_BYTE, data);
        textureWidth = width;
        textureHeight = height;
      } else {
        g.texSubImage2D(g.TEXTURE_2D, 0, 0, 0, width, height, g.RGBA, g.UNSIGNED_BYTE, data);
      }
      g.drawArrays(g.TRIANGLE_STRIP, 0, 4);
    },
  };
}

/** 2D canvas fallback; expects opaque RGBA (alpha already 255). */
export function create2dDrawer(canvas: HTMLCanvasElement): FrameDrawer | null {
  const context = canvas.getContext('2d', { alpha: false });
  if (!context) return null;
  return {
    draw(pixels, width, height) {
      if (canvas.width !== width || canvas.height !== height) {
        canvas.width = width;
        canvas.height = height;
      }
      const rgba = new Uint8ClampedArray(pixels.buffer as ArrayBuffer, pixels.byteOffset, width * height * 4);
      context.putImageData(new ImageData(rgba, width, height), 0, 0);
    },
  };
}

/** Canvas size in device pixels, capped at 1080p; width stays a multiple of 16. */
export function frameSize(
  cssWidth: number,
  cssHeight: number,
  pixelRatio: number,
): { width: number; height: number } {
  const w = Math.max(1, cssWidth * pixelRatio);
  const h = Math.max(1, cssHeight * pixelRatio);
  const scale = Math.min(1, 1920 / w, 1080 / h);
  return {
    width: Math.max(16, Math.floor((w * scale) / 16) * 16),
    height: Math.max(16, Math.round(h * scale)),
  };
}
