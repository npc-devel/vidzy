#version 320 es
layout (location = 0) in lowp vec2 aPos;
layout (location = 1) in lowp vec2 aTexCoords;

out lowp vec2 TexCoords;

void main()
{
    gl_Position = vec4(aPos.x, aPos.y, 0.0, 1.0);
    TexCoords = aTexCoords;
}
