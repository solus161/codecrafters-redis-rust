#!/bin/bash
{
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.0\r\n\$4\r\nhehe\r\n"
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.1\r\n\$4\r\nhoho\r\n"
  printf "*4\r\n\$4\r\nZADD\r\n\$3\r\nkey\r\n\$3\r\n3.1\r\n\$4\r\nzzzz\r\n"

  printf "*3\r\n\$5\r\nZRANK\r\n\$3\r\nkey\r\n\$4\r\nhehe\r\n"
  printf "*3\r\n\$5\r\nZRANK\r\n\$3\r\nkey\r\n\$4\r\nzzzz\r\n"
  
  printf "*4\r\n\$6\r\nZRANGE\r\n\$3\r\nkey\r\n\$1\r\n2\r\n\$1\r\n0\r\n"

  printf "*5\r\n\$6\r\nGEOADD\r\n\$6\r\norange\r\n\$18\r\n-165.0295233036336\r\n\$18\r\n40.666486875636664\r\n\$9\r\nblueberry\r\n"

  # GEOADD places 11.5030378 48.164271 "Munich"
  # GEOADD places 2.2944692 48.8584625 "Paris"
  # GEOADD places -0.0884948 51.506479 "London"
  printf "*5\r\n\$6\r\nGEOADD\r\n\$6\r\nplaces\r\n\$10\r\n11.5030378\r\n\$9\r\n48.164271\r\n\$6\r\nMunich\r\n"
  printf "*5\r\n\$6\r\nGEOADD\r\n\$6\r\nplaces\r\n\$9\r\n2.2944692\r\n\$10\r\n48.8584625\r\n\$5\r\nParis\r\n"
  printf "*5\r\n\$6\r\nGEOADD\r\n\$6\r\nplaces\r\n\$10\r\n-0.0884948\r\n\$9\r\n51.506479\r\n\$6\r\nLondon\r\n"

  # GEOSEARCH places FROMLONLAT 2 48 BYRADIUS 100000 m
  printf "*8\r\n\$9\r\nGEOSEARCH\r\n\$6\r\nplaces\r\n\$10\r\nFROMLONLAT\r\n\$1\r\n2\r\n\$2\r\n48\r\n\$8\r\nBYRADIUS\r\n\$6\r\n100000\r\n\$1\r\nm\r\n"
} | nc localhost 6379
