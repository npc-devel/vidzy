#version 320 es
out lowp vec4 FragColor;
  
in lowp vec2 TexCoords;

uniform sampler2D screenTexture;

void main()
{ 
    FragColor = texture(screenTexture, TexCoords);
}
