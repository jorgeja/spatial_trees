# Spatial trees
A library with a few different spatial trees. Main purpose is for region-subdivision in game development.
Implements Quadtree, Octtree and a "Cubetree" called PlanetTree which is a cube where all the sides have a quadtree. 

The trees do not store any custom data, but the node-storage is a slotmap, so it is easy to use a "SecondaryMap" to store additional data for each node. 


 